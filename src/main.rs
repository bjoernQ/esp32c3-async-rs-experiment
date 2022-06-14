#![no_std]
#![no_main]
#![feature(generic_associated_types)]

mod async_hal;
mod executor;

use embedded_hal_1 as embedded_hal;

use async_hal::AsyncPin;
use embedded_hal_async::digital::Wait;
use esp32c3_hal::{
    clock::ClockControl, ehal::digital::v2::InputPin, pac::Peripherals, prelude::*, RtcCntl, Timer,
    IO,
};
use esp_backtrace as _;
use esp_println::println;
use riscv_rt::entry;

use crate::executor::run_to_completion;

#[entry]
fn main() -> ! {
    println!("Hello!");
    run_to_completion(async_main());
    println!("That's it!");

    loop {}
}

async fn async_main() {
    println!("Hello, again!");

    let peripherals = Peripherals::take().unwrap();
    let system = peripherals.SYSTEM.split();
    let _clocks = ClockControl::boot_defaults(system.clock_control).freeze();
    let mut rtc_cntl = RtcCntl::new(peripherals.RTC_CNTL);
    let mut timer0 = Timer::new(peripherals.TIMG0);
    let mut timer1 = Timer::new(peripherals.TIMG1);

    rtc_cntl.set_super_wdt_enable(false);
    rtc_cntl.set_wdt_enable(false);
    timer0.disable();
    timer1.disable();

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let boot_button = io.pins.gpio9.into_pull_down_input();
    let boot_button = AsyncPin::from_pin(boot_button, 9);
    //handle_boot_button(boot_button).await;

    let io1_button = io.pins.gpio1.into_pull_down_input();
    let io1_button = AsyncPin::from_pin(io1_button, 1);

    //handle_second_button(io1_button).await;

    futures::join!(
        handle_boot_button(boot_button),
        handle_second_button(io1_button)
    );

    loop {}
}

async fn handle_boot_button<P>(mut button: AsyncPin<P>)
where
    P: InputPin + esp_hal_common::Pin,
{
    loop {
        button.wait_for_low().await.unwrap();
        println!("Boot button pressed!");
    }
}

async fn handle_second_button<P>(mut button: AsyncPin<P>)
where
    P: InputPin + esp_hal_common::Pin,
{
    loop {
        button.wait_for_high().await.unwrap();
        println!("Button on IO1 pressed!");
    }
}
