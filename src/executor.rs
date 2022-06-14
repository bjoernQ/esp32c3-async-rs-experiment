use core::pin::Pin;
use esp_hal_common::{interrupt, Cpu};
use futures::Future;

pub static mut ROOT_EXECUTOR: Option<Executor> = None;

pub struct Executor<'a> {
    root_task: Pin<&'a mut dyn Future<Output = ()>>,
}

impl<'a> Executor<'a> {
    pub fn new(task: &'a mut dyn Future<Output = ()>) -> Executor<'a> {
        let tsk = unsafe { Pin::new_unchecked(task) };
        Executor { root_task: tsk }
    }

    pub fn run_to_completion(&mut self) {
        unsafe {
            let interrupt_number = 50; // SW_INTERRUPT_0
            let cpu_interrupt_number = 13;
            let intr = &*esp32c3_hal::pac::INTERRUPT_CORE0::PTR;
            let intr_map_base = intr.mac_intr_map.as_ptr();
            intr_map_base
                .offset(interrupt_number)
                .write_volatile(cpu_interrupt_number as u32);

            // enable interrupt
            intr.cpu_int_enable
                .modify(|r, w| w.bits((1 << cpu_interrupt_number) | r.bits()));
        }

        interrupt::set_kind(
            Cpu::ProCpu,
            interrupt::CpuInterrupt::Interrupt13,
            interrupt::InterruptKind::Level,
        );
        interrupt::set_priority(
            Cpu::ProCpu,
            interrupt::CpuInterrupt::Interrupt13,
            interrupt::Priority::Priority10,
        );

        loop {
            unsafe {
                (*&esp32c3_hal::pac::SYSTEM::PTR)
                    .as_ref()
                    .unwrap()
                    .cpu_intr_from_cpu_0
                    .write(|w| w.bits(1));
                riscv::asm::wfi();
            }
        }
    }
}

pub fn yield_to_async_executor() {
    unsafe {
        (*&esp32c3_hal::pac::SYSTEM::PTR)
            .as_ref()
            .unwrap()
            .cpu_intr_from_cpu_0
            .write(|w| w.bits(1));
    }
}

#[no_mangle]
pub fn interrupt13() {
    static VTABLE: core::task::RawWakerVTable = {
        unsafe fn clone(p: *const ()) -> core::task::RawWaker {
            core::task::RawWaker::new(p, &VTABLE)
        }
        unsafe fn wake(p: *const ()) {
            wake_by_ref(p)
        }
        unsafe fn wake_by_ref(_p: *const ()) {
            yield_to_async_executor();
        }
        unsafe fn drop(_: *const ()) {
            // no-op
        }

        core::task::RawWakerVTable::new(clone, wake, wake_by_ref, drop)
    };

    let exec = unsafe { ROOT_EXECUTOR.as_mut() }.unwrap();
    let task = &mut exec.root_task;
    let waker = core::task::RawWaker::new(core::ptr::null(), &VTABLE);
    let waker = unsafe { core::task::Waker::from_raw(waker) };
    let mut ctx = core::task::Context::from_waker(&waker);

    if task.as_mut().poll(&mut ctx).is_ready() {
        panic!("The root task should never finish!");
    }

    unsafe {
        (*&esp32c3_hal::pac::SYSTEM::PTR)
            .as_ref()
            .unwrap()
            .cpu_intr_from_cpu_0
            .write(|w| w.bits(0));
    }
    interrupt::clear(Cpu::ProCpu, interrupt::CpuInterrupt::Interrupt3);
}
