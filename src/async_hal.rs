use core::task::{Context, Poll, Waker};

use esp32c3_hal::interrupt::Priority;
use esp32c3_hal::macros::interrupt;
use esp32c3_hal::pac;
use esp32c3_hal::{ehal::digital::v2::InputPin, interrupt, pac::Interrupt, Cpu};
use heapless::FnvIndexMap;

pub trait PinAsyncExt<P>
where
    P: InputPin + esp_hal_common::Pin,
{
    fn into_async(self) -> AsyncPin<P>;
}

impl<T: InputPin + esp_hal_common::Pin> PinAsyncExt<T> for T {
    fn into_async(self) -> AsyncPin<T> {
        AsyncPin::from_pin(self)
    }
}

#[derive(Debug, Clone)]
pub struct AwaitingPin {
    number: u8,
    signaled: usize,
    waker: Option<Waker>,
}

static mut AWAITING: FnvIndexMap<u8, AwaitingPin, 16> = FnvIndexMap::new();

pub struct AsyncPin<P>
where
    P: InputPin,
{
    pin: P,
    number: u8,
}

impl<P> AsyncPin<P>
where
    P: InputPin + esp_hal_common::Pin,
{
    // we should have a function to know the pin number
    pub fn from_pin(pin: P) -> AsyncPin<P> {
        interrupt::enable(Interrupt::GPIO, Priority::Priority1).unwrap();

        let number = pin.number();

        AsyncPin { pin, number }
    }

    pub fn free(self) -> P {
        self.pin
    }
}

#[derive(Debug)]
pub enum AsyncError {
    Unknown,
}

impl<P> crate::embedded_hal::digital::ErrorType for AsyncPin<P>
where
    P: InputPin + esp_hal_common::gpio::Pin,
{
    type Error = AsyncError;
}

impl<P> embedded_hal_async::digital::Wait for AsyncPin<P>
where
    P: InputPin + esp_hal_common::gpio::Pin,
{
    type WaitForHighFuture<'a>  = Signal
    where
        Self: 'a;

    fn wait_for_high<'a>(&'a mut self) -> Self::WaitForHighFuture<'a> {
        riscv::interrupt::free(|_cs| unsafe {
            let awaiting = AwaitingPin {
                number: self.number,
                signaled: 0,
                waker: None,
            };

            AWAITING.insert(self.number, awaiting).unwrap();
        });
        self.pin.listen(esp_hal_common::Event::HighLevel);
        Signal::new(self.number)
    }

    type WaitForLowFuture<'a>  = Signal
    where
        Self: 'a;

    fn wait_for_low<'a>(&'a mut self) -> Self::WaitForLowFuture<'a> {
        riscv::interrupt::free(|_cs| unsafe {
            let awaiting = AwaitingPin {
                number: self.number,
                signaled: 0,
                waker: None,
            };

            AWAITING.insert(self.number, awaiting).unwrap();
        });
        self.pin.listen(esp_hal_common::Event::LowLevel);
        Signal::new(self.number)
    }

    type WaitForRisingEdgeFuture<'a>  = Signal
    where
        Self: 'a;

    fn wait_for_rising_edge<'a>(&'a mut self) -> Self::WaitForRisingEdgeFuture<'a> {
        riscv::interrupt::free(|_cs| unsafe {
            let awaiting = AwaitingPin {
                number: self.number,
                signaled: 0,
                waker: None,
            };

            AWAITING.insert(self.number, awaiting).unwrap();
        });
        self.pin.listen(esp_hal_common::Event::RisingEdge);
        Signal::new(self.number)
    }

    type WaitForFallingEdgeFuture<'a>  = Signal
    where
        Self: 'a;

    fn wait_for_falling_edge<'a>(&'a mut self) -> Self::WaitForFallingEdgeFuture<'a> {
        riscv::interrupt::free(|_cs| unsafe {
            let awaiting = AwaitingPin {
                number: self.number,
                signaled: 0,
                waker: None,
            };

            AWAITING.insert(self.number, awaiting).unwrap();
        });
        self.pin.listen(esp_hal_common::Event::FallingEdge);
        Signal::new(self.number)
    }

    type WaitForAnyEdgeFuture<'a>  = Signal
    where
        Self: 'a;

    fn wait_for_any_edge<'a>(&'a mut self) -> Self::WaitForAnyEdgeFuture<'a> {
        riscv::interrupt::free(|_cs| unsafe {
            let awaiting = AwaitingPin {
                number: self.number,
                signaled: 0,
                waker: None,
            };

            AWAITING.insert(self.number, awaiting).unwrap();
        });
        self.pin.listen(esp_hal_common::Event::AnyEdge);
        Signal::new(self.number)
    }
}

pub struct Signal {
    pin_number: u8,
}

impl Signal {
    pub fn new(pin_number: u8) -> Self {
        Self { pin_number }
    }
}

impl futures::Future for Signal {
    type Output = Result<(), AsyncError>;

    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let done = riscv::interrupt::free(|_cs| unsafe {
            if let Some(awaiting) = AWAITING.get(&self.pin_number) {
                if awaiting.signaled > 0 {
                    return true;
                }
            }
            false
        });

        if done {
            riscv::interrupt::free(|_cs| unsafe {
                AWAITING.remove(&self.pin_number);
            });
            Poll::Ready(Ok(()))
        } else {
            riscv::interrupt::free(|_cs| unsafe {
                if let Some(awaiting) = AWAITING.get_mut(&self.pin_number) {
                    awaiting.waker = Some(cx.waker().clone());
                }
            });
            Poll::Pending
        }
    }
}

#[interrupt]
fn GPIO() {
    let mut waker_to_call = None;
    riscv::interrupt::free(|_cs| unsafe {
        let intrs = (*esp32c3_hal::pac::GPIO::PTR).pcpu_int.read().bits();

        for (_, pin) in AWAITING.iter_mut() {
            if 1 << pin.number & intrs != 0 {
                pin.signaled = 1;
                if let Some(ref waker) = pin.waker {
                    waker_to_call = Some(waker.clone());
                }
            }
        }

        (*esp32c3_hal::pac::GPIO::PTR)
            .status_w1tc
            .write(|w| w.bits(intrs));

        interrupt::clear(Cpu::ProCpu, interrupt::CpuInterrupt::Interrupt3);
    });

    if waker_to_call.is_some() {
        (waker_to_call.unwrap()).wake();
    }
}
