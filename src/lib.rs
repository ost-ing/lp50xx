//! # lp50xx library
//! A small library for using the Texas Instruments LP5009 and LP5012 LED drivers

#![no_std]
#![deny(warnings)]

use core::marker::PhantomData;
use embedded_hal::digital::v2::OutputPin;

pub enum Error {
    CommError,
    NoInterfaceDefined,
}

pub enum Model {
    LP5009,
    LP5012,
}

impl Model {
    fn get_pin_count(&mut self) -> u8 {
        match *self {
            Model::LP5009 => 9,
            Model::LP5012 => 12,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Address {
    Broadcast,
    Independent(u8),
}

impl Address {
    pub fn into_u8(&self) -> u8 {
        match self {
            Address::Broadcast => 0x0C,
            Address::Independent(address) => {
                if *address > 3 {
                    panic!("LP50XX only supports 3 dedicated addresses")
                }
                return 0b00001100 | address;
            }
        }
    }
}

pub struct BasicMode {}

impl BasicMode {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct ColorMode {}

impl ColorMode {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct MonochromaticMode {}

impl MonochromaticMode {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct LP50xx<MODE, I2C, EN> {
    /// I2C interface, used specifically for blocking writes to the LP50XX
    interface: Option<(I2C, EN)>,
    /// Asynchronous transfer callback, useful for transferring data to a static DMA buffer or queue
    /// When the blocking I2C interface is provided, this transfer_callback value is ignored
    transfer_callback: Option<fn(address: Address, data: &[u8])>,
    /// Continuous addressing allows intuitive numbering of banks/leds when multiple LP50XX chips are used
    /// in a daisy-chain configuration. For example, for the LP5009 if specifying the 9th led, the address will be 0x00
    /// but when specifying the 10th led, the address will be 0x01 (the next chip address)
    continuous_addressing: bool,
    /// Chip select address (ignored when continuous addressing is set to true)
    active_address: Address,
    /// The Display Mode of the LP50XX, which modifies the API for intuitive use for RGB Color mode or Monochromatic mode
    mode: PhantomData<MODE>,
    /// Model, can either be the LP5009 (9 pin) or LP5012 (12 pin)
    model: Model,
}

impl<I2C, EN> LP50xx<BasicMode, I2C, EN> {
    pub fn init_with_i2c(
        model: Model,
        i2c: I2C,
        en: EN,
    ) -> Self {
        Self {
            interface: Some((i2c, en)),
            transfer_callback: None,
            model,
            active_address: Address::Broadcast,
            continuous_addressing: true,
            mode: PhantomData,
        }
    }

    pub fn init_with_callback(model: Model, callback: fn(address: Address, data: &[u8])) -> Self {
        Self {
            interface: None,
            transfer_callback: Some(callback),
            model,
            active_address: Address::Broadcast,
            continuous_addressing: true,
            mode: PhantomData,
        }
    }

    /// Set continuous addressing
    pub fn set_continuous_addressing(&mut self, state: bool) {
        self.continuous_addressing = state;
    }

    pub fn set_active_address(&mut self, address: Address) {
        self.active_address = address;
    }
}

impl<MODE, I2C, EN> LP50xx<MODE, I2C, EN> where
    I2C: embedded_hal::blocking::i2c::Write<u8>,
    EN: OutputPin,
{
    pub fn into_color_mode(self) -> LP50xx<ColorMode, I2C, EN> {
        self.into_mode::<ColorMode>()
    }

    pub fn into_monochromatic_mode(self) -> LP50xx<MonochromaticMode, I2C, EN> {
        self.into_mode::<MonochromaticMode>()
    }

    fn into_mode<MODE2>(self) -> LP50xx<MODE2, I2C, EN> {
        LP50xx {
            interface: self.interface,
            transfer_callback: self.transfer_callback,
            active_address: self.active_address,
            model: self.model,
            continuous_addressing: self.continuous_addressing,
            mode: PhantomData,
        }
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        // If there is an i2c interface provided, utilize it in a blocking fashion
        if self.interface.is_some() {
            self.interface
                .as_mut()
                .unwrap()
                .0
                .write(addr.into_u8(), data)
                .map_err(|_| Error::CommError)?;
        } else if self.transfer_callback.is_some() {
            self.transfer_callback.unwrap()(addr, data);
            return Ok({});
        }
        return Err(Error::NoInterfaceDefined);
    }

    pub fn enable(&mut self) -> Result<(), Error> {
        self.write(Address::Broadcast, &[0x00, 0b01000000])
    }

    pub fn configure(
        &mut self,
        log_scale: bool,
        power_save: bool,
        auto_incr: bool,
        pwm_dithering: bool,
        max_current_option: bool,
        global_off: bool,
    ) -> Result<(), Error> {
        let value: u8 = 0x00
            | (log_scale as u8) << 5
            | (power_save as u8) << 4
            | (auto_incr as u8) << 3
            | (pwm_dithering as u8) << 2
            | (max_current_option as u8) << 1
            | (global_off as u8) << 0;

        self.write(Address::Broadcast, &[0x01, value])
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        self.write(Address::Broadcast, &[0x17, 0xff])
    }
}

// Color Mode

impl<I2C, EN> LP50xx<ColorMode, I2C, EN>  
where
    I2C: embedded_hal::blocking::i2c::Write<u8>,
    EN: OutputPin 
{
    pub fn set(
        &mut self,
        address: Address,
        channel: u8,
        (brightness, [r, g, b]): (u8, [u8; 3]),
    ) -> Result<(), Error> {
        let bright_addr = 0x07 + channel as u8;
        let color_addr = 0x0b + (channel as u8) * 3;
        self.write(address, &[bright_addr, brightness])?;
        self.write(address, &[color_addr, r, g, b])?;
        Ok(())
    }
}

// Monochromatic Mode

impl<I2C, EN> LP50xx<MonochromaticMode, I2C, EN>  
where
    I2C: embedded_hal::blocking::i2c::Write<u8>,
    EN: OutputPin
{
    pub fn set(&mut self, led: u8, value: u8) -> Result<(), Error> {
        if !self.continuous_addressing && led > self.model.get_pin_count() - 1 {
            panic!("Specified LED is not supported");
        }

        // In monochromatic mode, brightness is no longer applicable
        let led_base_address = 0x0B;

        let address = if self.continuous_addressing {
            let pin_count = self.model.get_pin_count() - 1;
            let offset = if led > pin_count && led < (pin_count * 2) {
                0x01
            } else if led >= (pin_count * 2) {
                0x02
            } else {
                0x00
            };

            Address::Independent(offset)
        } else {
            self.active_address
        };

        self.write(address, &[led_base_address + led, value])?;
        Ok(())
    }
}
