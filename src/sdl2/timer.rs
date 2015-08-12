use libc::{uint32_t, c_void};
use std::marker::PhantomData;
use std::mem;
use sys::timer as ll;

use TimerSubsystem;

impl TimerSubsystem {
    /// Constructs a new timer using the boxed closure `callback`.
    ///
    /// The timer is started immediately, it will be cancelled either:
    ///
    /// * when the timer is dropped
    /// * or when the callback returns a non-positive continuation interval
    pub fn add_timer<'b, 'c>(&'b self, delay: u32, callback: TimerCallback<'c>) -> Timer<'b, 'c> {
        unsafe {
            let callback = Box::new(callback);
            let timer_id = ll::SDL_AddTimer(delay,
                                            Some(c_timer_callback),
                                            mem::transmute_copy(&callback));

            Timer {
                callback: Some(callback),
                raw: timer_id,
                _marker: PhantomData
            }
        }
    }

    /// Gets the number of milliseconds elapsed since the timer subsystem was initialized.
    ///
    /// It's recommended that you use another library for timekeeping, such as `time`.
    pub fn ticks(&mut self) -> u32 {
        // Google says this is probably not thread-safe (TODO: prove/disprove this).
        unsafe { ll::SDL_GetTicks() }
    }

    /// Sleeps the current thread for the specified amount of milliseconds.
    ///
    /// It's recommended that you use `std::thread::sleep_ms()` instead.
    pub fn delay(&mut self, ms: u32) {
        // Google says this is probably not thread-safe (TODO: prove/disprove this).
        unsafe { ll::SDL_Delay(ms) }
    }

    pub fn performance_counter(&self) -> u64 {
        unsafe { ll::SDL_GetPerformanceCounter() }
    }

    pub fn performance_frequency(&self) -> u64 {
        unsafe { ll::SDL_GetPerformanceFrequency() }
    }
}

pub type TimerCallback<'a> = Box<FnMut() -> u32+'a+Sync>;

pub struct Timer<'b, 'a> {
    callback: Option<Box<TimerCallback<'a>>>,
    raw: ll::SDL_TimerID,
    _marker: PhantomData<&'b ()>
}

impl<'b, 'a> Timer<'b, 'a> {
    /// Returns the closure as a trait-object and cancels the timer
    /// by consuming it...
    pub fn into_inner(mut self) -> TimerCallback<'a> {
        *self.callback.take().unwrap()
    }
}

impl<'b, 'a> Drop for Timer<'b, 'a> {
    #[inline]
    fn drop(&mut self) {
        // SDL_RemoveTimer returns SDL_FALSE if the timer wasn't found (impossible),
        // or the timer has been cancelled via the callback (possible).
        // The timer being cancelled isn't an issue, so we ignore the result.
        unsafe { ll::SDL_RemoveTimer(self.raw) };
    }
}

extern "C" fn c_timer_callback(_interval: u32, param: *mut c_void) -> uint32_t {
    unsafe {
        let f: *mut Box<Fn() -> u32> = mem::transmute(param);
        (*f)() as uint32_t
    }
}


#[cfg(test)]
fn test_timer_runs_multiple_times() {
    use std::sync::{Arc, Mutex};
    let sdl_context = ::sdl::init().unwrap();
    let timer_subsystem = sdl_context.timer().unwrap();

    let local_num = Arc::new(Mutex::new(0));
    let timer_num = local_num.clone();

    let _timer = timer_subsystem.add_timer(20, Box::new(|| {
        // increment up to 10 times (0 -> 9)
        // tick again in 100ms after each increment
        //
        let mut num = timer_num.lock().unwrap();
        if *num < 9 {
            *num += 1;
            20
        } else { 0 }
    }));

    ::std::thread::sleep_ms(250);              // tick the timer at least 10 times w/ 200ms of "buffer"
    let num = local_num.lock().unwrap(); // read the number back
    assert_eq!(*num, 9);                 // it should have incremented at least 10 times...
}

#[cfg(test)]
fn test_timer_runs_at_least_once() {
    use std::sync::{Arc, Mutex};
    let sdl_context = ::sdl::init().unwrap();
    let timer_subsystem = sdl_context.timer().unwrap();

    let local_flag = Arc::new(Mutex::new(false));
    let timer_flag = local_flag.clone();

    let _timer = timer_subsystem.add_timer(20, Box::new(|| {
        let mut flag = timer_flag.lock().unwrap();
        *flag = true; 0
    }));

    ::std::thread::sleep_ms(50);
    let flag = local_flag.lock().unwrap();
    assert_eq!(*flag, true);
}

#[cfg(test)]
fn test_timer_can_be_recreated() {
    use std::sync::{Arc, Mutex};
    let sdl_context = ::sdl::init().unwrap();
    let timer_subsystem = sdl_context.timer().unwrap();

    let local_num = Arc::new(Mutex::new(0));
    let timer_num = local_num.clone();

    // run the timer once and reclaim its closure
    let timer_1 = timer_subsystem.add_timer(20, Box::new(move|| {
        let mut num = timer_num.lock().unwrap();
        *num += 1; // increment the number
        0          // do not run timer again
    }));

    // reclaim closure after timer runs
    ::std::thread::sleep_ms(50);
    let closure = timer_1.into_inner();

    // create a second timer and increment again
    let _timer_2 = timer_subsystem.add_timer(20, closure);
    ::std::thread::sleep_ms(50);

    // check that timer was incremented twice
    let num = local_num.lock().unwrap();
    assert_eq!(*num, 2);
}

#[test]
fn test_timer() {
    test_timer_runs_multiple_times();
    test_timer_runs_at_least_once();
    test_timer_can_be_recreated();
}
