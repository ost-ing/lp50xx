//! # lp50xx library
//! A small library for using the Texas Instruments LP5009 and LP5012 LED drivers

#![no_std]
#![deny(warnings)]

use core::marker::PhantomData;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;

#[derive(Debug)]
pub enum Error {
    /// Generic communication Error with blocking I2C
    CommError,
    /// Neither the I2C or asynchronous transfer callback defined
    NoInterfaceDefined,
    /// An error setting the Enable pin high or low
    EnableLine,
}

/// Supported Texas Instruments LP50XX models
#[derive(Clone, Copy)]
pub enum Model {
    /// 9 pin controller
    LP5009,
    /// 12 pin controller
    LP5012,
}

impl Model {
    /// Get the pin count for the Model
    fn get_pin_count(&self) -> u8 {
        match *self {
            Model::LP5009 => 9,
            Model::LP5012 => 12,
        }
    }
}

/// The chip select communication address
/// The addressing is 7bit
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
    /// NOTE: The directional bit is not included in the addressing and should be included in the i2c driver implementation
    pub fn into_u8(self) -> u8 {
        match self {
            Address::Independent(address) => {
                if address > 3 {
                    panic!("LP50XX only supports 4 dedicated addresses, 0b00, 0b01, 0b10 or 0b11")
                }
                return 0b00010100 | address;
            }
            Address::Broadcast => 0b0001100,
        }
    }
}

/// Default Mode
pub struct DefaultMode {}

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
    transfer_callback: Option<fn(addr: Address, data: &[u8])>,
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
    /// Brightness factor. Note: Only used for monochromatic mode.
    brightness_factor: f32,
}

impl<I2C, EN> LP50xx<DefaultMode, I2C, EN>
where
    EN: OutputPin,
{
    /// Initialize the LP50xx with a dedicated blocking i2c interface
    /// * `model` - The model of the LP50xx
    /// * `i2c` - I2C interface for blocking tranmission
    /// * `en` - The enable line
    pub fn init_with_i2c(model: Model, i2c: I2C, mut en: EN) -> Self {
        en.set_low().ok();

        Self {
            interface: Some(i2c),
            enable: en,
            transfer_callback: None,
            model,
            active_address: Address::Broadcast,
            continuous_addressing: true,
            mode: PhantomData,
            brightness_factor: 1.0,
        }
    }

    /// Initialize the LP50xx with a flexible asynchronous callback interface
    /// * `model` - The model of the LP50xx
    /// * `en` - The enable line
    /// * `callback` - Callback for custom transmission of the address and dataframe.
    pub fn init_with_callback(
        model: Model,
        mut en: EN,
        callback: fn(addr: Address, data: &[u8]),
    ) -> Self {
        en.set_low().ok();

        Self {
            interface: None,
            enable: en,
            transfer_callback: Some(callback),
            model,
            active_address: Address::Broadcast,
            continuous_addressing: true,
            mode: PhantomData,
            brightness_factor: 1.0,
        }
    }

    /// Set continuous addressing
    /// * `state` - Continuous addressing enable
    pub fn set_continuous_addressing(&mut self, state: bool) {
        self.continuous_addressing = state;
    }

    /// Set the active chip address: Broadcast, 0b00, 0b01, 0b10 or 0b11.
    /// * `address` - Address of the active LP50xx
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
    I2C: embedded_hal::i2c::I2c,
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
            brightness_factor: 1.0,
        }
    }

    /// Write data to the desired interface. If the i2C interface is provided,
    /// it will perform a blocking call to I2C and return the result,
    /// if I2C is not provided, then the asynchronous transfer callback is executed
    /// * `addr` - Address of the LP50xx
    /// * `data` - The data payload to be sent
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

    /// Reset the LP50xx
    /// Currently resetting is only available for Broadcast
    /// * `delay` - delay provider
    pub fn reset<DELAY>(&mut self, delay: &mut DELAY) -> Result<(), Error>
    where
        DELAY: DelayNs,
    {
        self.write(Address::Broadcast, &[0x17, 0xff])?;
        delay.delay_ms(1);
        self.enable.set_low().map_err(|_| Error::EnableLine)?;
        delay.delay_ms(10);
        self.enable.set_high().map_err(|_| Error::EnableLine)?;
        delay.delay_ms(10);
        Ok(())
    }

    /// Enable the LP50xx, this must be executed prior to any commands sent to the LP50xx
    /// * `delay` - delay provider
    pub fn enable<DELAY>(&mut self, delay: &mut DELAY) -> Result<(), Error>
    where
        DELAY: DelayNs,
    {
        self.enable.set_low().map_err(|_| Error::EnableLine)?;
        delay.delay_ms(1);
        self.enable.set_high().map_err(|_| Error::EnableLine)?;
        delay.delay_ms(10);
        self.write(Address::Broadcast, &[0x00, 0b01000000])
    }

    /// Configure the LP50xx. For information regarding each of these settings, please consult the datasheet.
    /// Currently configuring is only available for Broadcast
    /// * `log_scale` - Logarithmic scale dimming curve
    /// * `power_save` - Automatic power-saving mode enabled
    /// * `auto_incr` - The auto-increment feature allows writing or reading several consecutive registers within one transmission.
    /// * `pwm_dithering` - PWM dithering mode enabled
    /// * `max_current_option` - Output maximum current enable: IMAX = 35 mA, disable: IMAX = 25.5mA
    /// * `global_off` - Shut down all LEDs when enabled
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
}

// Color Mode

impl<I2C, EN> LP50xx<ColorMode, I2C, EN>
where
    I2C: embedded_hal::i2c::I2c,
    EN: OutputPin,
{
    /// Set the channel brightness and RGB values
    pub fn set(
        &mut self,
        mut channel: u8,
        (brightness, [r, g, b]): (u8, [u8; 3]),
    ) -> Result<(), Error> {
        if channel < 1 {
            panic!("Specified Channel index must be greater than 0");
        }

        channel = channel - 1;

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
    I2C: embedded_hal::i2c::I2c,
    EN: OutputPin,
{
    /// Set the brightness factor which will dim the output
    /// The maximum value is 1.0 (100%) and the minimum is 0.01 (1%)
    /// * `factor` - Brightness factor
    pub fn set_brightness_factor(&mut self, mut factor: f32) {
        if factor < 0.01 {
            factor = 0.01;
        } else if factor > 1.0 {
            factor = 1.0;
        }
        self.brightness_factor = factor;
    }

    /// Get the configured brightness factor
    pub fn brightness_factor(&self) -> f32 {
        self.brightness_factor
    }

    /// Set the desired LED value
    /// * `led` - the LED index beginning at 1
    /// * `value` - luminosity value
    pub fn set(&mut self, led: u8, value: u8) -> Result<(), Error> {
        if led == 0 {
            panic!("Specified LED index must be greater than 0");
        }
        if !self.continuous_addressing && led > self.model.get_pin_count() {
            panic!("Specified LED is not supported");
        }

        // In monochromatic mode, brightness is no longer applicable
        let led_base_address = 0x0B;

        let (address, pin_offset) = if self.continuous_addressing {
            let addr_offset = get_led_address_offset(led, self.model);
            let addr = Address::Independent(addr_offset);
            let pin_offset = led - (addr_offset * self.model.get_pin_count());
            (addr, pin_offset)
        } else {
            (self.active_address, led)
        };

        let result = (value as f32 * self.brightness_factor) as u8;

        self.write(address, &[led_base_address + (pin_offset - 1), result])?;
        Ok(())
    }
}

/// Get the led offset address for the given led index and the model
/// * `led_index` - the LED index beginning at 1
/// * `model` - Model number of the LP50xx
fn get_led_address_offset(led_index: u8, model: Model) -> u8 {
    let length = model.get_pin_count();

    if led_index <= length {
        return 0x00;
    }
    if led_index > length && led_index <= (length * 2) {
        return 0x01;
    }

    return 0x02;
}

#[cfg(test)]
mod tests {
    #[test]
    fn correct_led_address_offset() {
        let offset = super::get_led_address_offset(1, super::Model::LP5012);
        assert_eq!(offset, 0x00);
        let offset = super::get_led_address_offset(12, super::Model::LP5012);
        assert_eq!(offset, 0x00);
        let offset = super::get_led_address_offset(13, super::Model::LP5012);
        assert_eq!(offset, 0x01);
        let offset = super::get_led_address_offset(24, super::Model::LP5012);
        assert_eq!(offset, 0x01);
        let offset = super::get_led_address_offset(25, super::Model::LP5012);
        assert_eq!(offset, 0x02);
    }
}
