# lp50xx
Embedded driver for the Texas Instruments LP5009 and LP5012 LED drivers

- https://crates.io/crates/lp50xx

## description
The LP50xx device is an 9- or 12-channel constant current sink LED driver. The LP50xx device includes integrated color mixing and brightness control, and pre-configuration simplifies the software coding process. Integrated 12-bit, 29-kHz PWM generators for each channel enable smooth, vivid color for LEDs, and eliminate audible noise.

## example
Examples are based on the `stm32h7xx_hal`.

```rust
// Initialize I2C pins, SCL, SDA
let scl = scl
    .into_alternate_af4()
    .internal_pull_up(true)
    .set_open_drain();
let sda = sda
    .into_alternate_af4()
    .internal_pull_up(true)
    .set_open_drain();

// Initialize the Enable line
let en = en.into_push_pull_output();

// Initialize 
let i2c: stm32h7xx_hal::i2c::I2c<I2C1> =
    i2c.i2c((scl, sda), 100.khz(), prec, clocks);

// Initialize with blocking I2C
let interface = LP50xx::init_with_i2c(Model::LP5012, i2c, en);
// Use the LP50xx in monochromatic mode
let mut monochromatic_controller = interface.into_monochromatic_mode();
// Enable it, this requires a delay provider
monochromatic_controller.enable(delay).ok();
// Set LED 5 to 255
monochromatic_controller.set(5, 0xFF).ok();

// The continuous addressing feature is enabled by default, allowing you to address
// multiple LP50xx devices on the same bus in an intuitive way.
// There are 12 LEDs available per LP5012, therefore the following command
// will illuminate the 1st LED on the on the second LP5012:
monochromatic_controller.set(13, 0xFF).ok();
// You can turn this feature off and set the active address manually also:
monochromatic_controller.set_continuous_addressing(false);
// Set the active address to be the second device
monochromatic_controller.set_active_address(0x01);
// And then turn off the same 1st LED on the second device
monochromatic_controller.set(1, 0x00).ok();

// Alternatively, if you are using RGB LEDs you can use the LP50xx in color mode
let mut color_controller = monochromatic_controller.into_color_mode();
// Set channel 1 brightness and RGB values
color_controller.set(1, (1, [255, 100, 95])).ok();

// Release the blocking i2c example to regain access to its underyling resources
let (_i2c, en) = color_controller.release();

// Additionally, if you need to integrate this driver with platform specific DMA controllers then
// a flexible callback can be used rather than blocking i2c
static mut DMA_BUFFER: [u8; 256] = [0; 256];
let interface = LP50xx::init_with_callback(Model::LP5012, en, |addr, data| unsafe {
    // Copy the data from the LP50xx into the DMA buffer for processing
    DMA_BUFFER[0..data.len()].copy_from_slice(data);
})
.into_monochromatic_mode();
```

## contributing
Feel free to create a ticket and a MR for any changes you would like to see in this library.
