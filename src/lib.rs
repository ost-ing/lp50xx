//! # lp50xx library
//! A small library for using the Texas Instruments LP5009 and LP5012 LED drivers

#![no_std]
#![deny(warnings)]

use core::marker::PhantomData;
use embedded_hal::digital::v2::OutputPin;

#[derive(Debug)]
pub enum Error {
    /// Communication Error with blocking I2C
    CommError,
    /// Neither the I2C or asynchronous transfer callback defined
    NoInterfaceDefined,
    /// An error setting the Enable pin high or low
    EnableLine,
}

/// Supported Texas Instruments LP50XX models
pub enum Model {
    /// 9 pin controller
    LP5009,
    /// 12 pin controller
    LP5012,
}

impl Model {
    /// Get the pin count for the Model
    fn get_pin_count(&mut self) -> u8 {
        match *self {
            Model::LP5009 => 9,
            Model::LP5012 => 12,
        }
    }
}

/// The chip select communication address
#[derive(Clone, Copy)]
pub enum Address {
    /// Broadcast the transferred data to all LP50XX chips on the I2C bus
    Broadcast,
    /// Send the transferred data specifically to one LP50XX chip on the I2C bus. This requires
    /// that the LP50XX is addressed properly by pulling the relevant pins high or low in the circuit.
    Independent(u8),
}

impl Address {
    /// Return the u8 payload data for the address specifier, this data can sent down the wire to the LP50XX to
    /// specifiy the desired chip
    pub fn into_u8(&self) -> u8 {
        match self {
            Address::Broadcast => 0x0C,
            Address::Independent(address) => {
                if *address > 4 {
                    panic!("LP50XX only supports 3 dedicated addresses, 0b00, 0b01, 0b10 or 0b11")
                }
                return 0b00001100 | address;
            }
        }
    }
}

/// Basic or default API for the LP50XX.
/// This currently does nothing and the user must choose either the ColorMode or MonochromaticMode APIs.
pub struct BasicMode {}

impl BasicMode {
    pub fn new() -> Self {
        Self {}
    }
}

/// ColorMode allows the user to configure the LEDs in fashion that is suitable if the LED supports RGB
pub struct ColorMode {}
impl ColorMode {
    pub fn new() -> Self {
        Self {}
    }
}

/// MonochromaticMode allows the user to configure the LEDs in a fashion that is suitable if the LEDs are monochromatic
pub struct MonochromaticMode {}

impl MonochromaticMode {
    pub fn new() -> Self {
        Self {}
    }
}

/// The LP50XX (LP5009 or LP5012) is a 9 or 12 pin LED controller by Texas Instruments
pub struct LP50xx<MODE, I2C, EN> {
    /// I2C interface, used specifically for blocking writes to the LP50XX
    interface: Option<I2C>,
    /// Enable line
    enable: EN,
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
    /// Initialize the LP50xx with a dedicated blocking i2c interface
    pub fn init_with_i2c(model: Model, i2c: I2C, en: EN) -> Self {
        Self {
            interface: Some(i2c),
            enable: en,
            transfer_callback: None,
            model,
            active_address: Address::Broadcast,
            continuous_addressing: true,
            mode: PhantomData,
        }
    }

    /// Initialize the LP50xx with a flexible asynchronous callback interface
    pub fn init_with_callback(
        model: Model,
        en: EN,
        callback: fn(address: Address, data: &[u8]),
    ) -> Self {
        Self {
            interface: None,
            enable: en,
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

    /// Set the active chip address: Broadcast, 0b00, 0b01, 0b10 or 0b11.
    pub fn set_active_address(&mut self, address: Address) {
        self.active_address = address;
    }

    /// Release underlying resources back to initiator
    pub fn release(self) -> (Option<I2C>, EN) {
        (self.interface, self.enable)
    }
}

impl<MODE, I2C, EN> LP50xx<MODE, I2C, EN>
where
    I2C: embedded_hal::blocking::i2c::Write,
    EN: OutputPin,
{
    /// Configure the LP50xx to be in color mode, which is most suitable if the target LEDs support RGB
    pub fn into_color_mode(self) -> LP50xx<ColorMode, I2C, EN> {
        self.into_mode::<ColorMode>()
    }

    /// Configure the LP50xx to be in monochromatic mode, which is most suitable if the target LEDs are monochromatic
    pub fn into_monochromatic_mode(self) -> LP50xx<MonochromaticMode, I2C, EN> {
        self.into_mode::<MonochromaticMode>()
    }

    /// Helper function to convert the struct appropriately
    fn into_mode<MODE2>(self) -> LP50xx<MODE2, I2C, EN> {
        LP50xx {
            interface: self.interface,
            enable: self.enable,
            transfer_callback: self.transfer_callback,
            active_address: self.active_address,
            model: self.model,
            continuous_addressing: self.continuous_addressing,
            mode: PhantomData,
        }
    }

    /// Write data to the desired interface. If the i2C interface is provided,
    /// it will perform a blocking call to I2C and return the result,
    /// if I2C is not provided, then the asynchronous transfer callback is executed
    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        // If there is an i2c interface provided, utilize it in a blocking fashion
        if self.interface.is_some() {
            self.interface
                .as_mut()
                .unwrap()
                .write(addr.into_u8(), data)
                .map_err(|_| Error::CommError)?;
            return Ok({});
        }

        if self.transfer_callback.is_some() {
            self.transfer_callback.unwrap()(addr, data);
            return Ok({});
        }

        return Err(Error::NoInterfaceDefined);
    }

    /// Enable the LP50xx, this must be executed prior to any commands sent to the LP50xx
    pub fn enable(&mut self) -> Result<(), Error> {
        self.enable.set_low().map_err(|_| Error::EnableLine)?;
        self.enable.set_high().map_err(|_| Error::EnableLine)?;

        self.write(Address::Broadcast, &[0x00, 0b01000000])
    }

    /// Configure the LP50xx. For information regarding each of these settings, please consult the datasheet.
    /// Currently configuring is only available for Broadcast
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

    /// Reset the LP50xx
    /// Currently resetting is only available for Broadcast
    pub fn reset(&mut self) -> Result<(), Error> {
        self.write(Address::Broadcast, &[0x17, 0xff])?;
        self.enable.set_low().map_err(|_| Error::EnableLine)?;
        self.enable.set_high().map_err(|_| Error::EnableLine)?;
        Ok(())
    }
}

// Color Mode

impl<I2C, EN> LP50xx<ColorMode, I2C, EN>
where
    I2C: embedded_hal::blocking::i2c::Write,
    EN: OutputPin,
{
    /// Set the channel brightness and RGB values
    pub fn set(
        &mut self,
        channel: u8,
        (brightness, [r, g, b]): (u8, [u8; 3]),
    ) -> Result<(), Error> {
        // TODO: Continuous Addressing feature

        let bright_addr = 0x07 + channel as u8;
        let color_addr = 0x0b + (channel as u8) * 3;
        self.write(self.active_address, &[bright_addr, brightness])?;
        self.write(self.active_address, &[color_addr, r, g, b])?;
        Ok(())
    }
}

// Monochromatic Mode

impl<I2C, EN> LP50xx<MonochromaticMode, I2C, EN>
where
    I2C: embedded_hal::blocking::i2c::Write,
    EN: OutputPin,
{
    /// Set the desired LED value
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
