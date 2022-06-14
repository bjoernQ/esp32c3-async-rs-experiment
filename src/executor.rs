use core::pin::Pin;
use futures::Future;

pub fn run_to_completion<T>(mut task: T)
where
    T: Future<Output = ()>,
{
    static VTABLE: core::task::RawWakerVTable = {
        unsafe fn clone(p: *const ()) -> core::task::RawWaker {
            core::task::RawWaker::new(p, &VTABLE)
        }
        unsafe fn wake(p: *const ()) {
            wake_by_ref(p)
        }
        unsafe fn wake_by_ref(p: *const ()) {
            // no-op
        }
        unsafe fn drop(_: *const ()) {
            // no-op
        }

        core::task::RawWakerVTable::new(clone, wake, wake_by_ref, drop)
    };

    let mut task = unsafe { Pin::new_unchecked(&mut task) };

    let waker = core::task::RawWaker::new(core::ptr::null(), &VTABLE);
    let waker = unsafe { core::task::Waker::from_raw(waker) };
    let mut ctx = core::task::Context::from_waker(&waker);

    loop {
        if task.as_mut().poll(&mut ctx).is_ready() {
            break;
        }

        unsafe {
            riscv::asm::wfi();
        }
    }
}
