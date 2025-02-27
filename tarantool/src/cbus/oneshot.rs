use super::{LCPipe, Message};
use crate::cbus::RecvError;
use crate::fiber::Cond;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A oneshot channel based on tarantool cbus. This a channel between any arbitrary thread and a cord.
/// Cord - a thread with `libev` event loop inside (typically tx thread).
struct Channel<T> {
    message: UnsafeCell<Option<T>>,
    /// Condition variable for synchronize consumer (cord) and producer,
    /// using an [`Arc`] instead of raw pointer cause there is a situation
    /// when channel dropped before cbus endpoint receive a cond
    cond: Arc<Cond>,
    /// Atomic flag, signaled that sender already have a data for receiver
    ready: AtomicBool,
}

unsafe impl<T> Sync for Channel<T> where T: Send {}

unsafe impl<T> Send for Channel<T> where T: Send {}

impl<T> Channel<T> {
    /// Create a new channel.
    fn new() -> Self {
        Self {
            message: UnsafeCell::new(None),
            ready: AtomicBool::new(false),
            cond: Arc::new(Cond::new()),
        }
    }
}

/// A sending-half of oneshot channel. Can be used in any context (tarantool cord or arbitrary thread).
/// Messages can be sent through this channel with [`Sender::send`].
///
/// If sender dropped before [`Sender::send`] is calling then [`EndpointReceiver::receive`] will return with [`RecvError::Disconnected`].
/// It is safe to drop sender when [`EndpointReceiver::receive`] is not calling.
pub struct Sender<T> {
    channel: Arc<Channel<T>>,
    pipe: Arc<LCPipe>,
}

/// Receiver part of oneshot channel. Must be used in cord context.
pub struct EndpointReceiver<T> {
    channel: Arc<Channel<T>>,
}

/// Creates a new oneshot channel, returning the sender/receiver halves with already created [`LCPipe`] instance.
/// This method is useful if you want to avoid any memory allocations.
/// Typically better use a [`channel`] method that create a new lcpipe instance,
/// lcpipe is pretty small structure so overhead is not big.
///
/// # Arguments
///
/// * `pipe`: lcpipe - a cbus communication channel
///
/// returns: (Sender<T>, Receiver<T>)
///
/// # Examples
///
/// ```no_run
/// #[cfg(feature = "picodata")] {
/// use std::sync::Arc;
/// use tarantool::cbus::oneshot;
/// use tarantool::cbus::LCPipe;
///
/// let pipe = LCPipe::new("some_endpoint");
/// let (sender, receiver) = oneshot::channel_on_pipe::<u8>(Arc::new(pipe));
/// }
/// ```
pub fn channel_on_pipe<T>(pipe: Arc<LCPipe>) -> (Sender<T>, EndpointReceiver<T>) {
    let channel = Arc::new(Channel::new());
    (
        Sender {
            channel: channel.clone(),
            pipe,
        },
        EndpointReceiver { channel },
    )
}

/// Creates a new oneshot channel, returning the sender/receiver halves. Please note that the receiver should only be used inside the cord.
///
/// # Arguments
///
/// * `cbus_endpoint`: cbus endpoint name. Note that the tx thread (or any other cord)
/// must have a fiber occupied by the endpoint cbus_loop.
///
/// returns: (Sender<T>, Receiver<T>)
///
/// # Examples
///
/// ```no_run
/// #[cfg(feature = "picodata")] {
/// use tarantool::cbus::oneshot;
/// let (sender, receiver) = oneshot::channel::<u8>("some_endpoint");
/// }
/// ```
pub fn channel<T>(cbus_endpoint: &str) -> (Sender<T>, EndpointReceiver<T>) {
    channel_on_pipe(Arc::new(LCPipe::new(cbus_endpoint)))
}

impl<T> Sender<T> {
    /// Attempts to send a value on this channel.
    ///
    /// # Arguments
    ///
    /// * `message`: message to send
    pub fn send(self, message: T) {
        unsafe { *self.channel.message.get() = Some(message) };
        self.channel.ready.store(true, Ordering::Release);
        // [`Sender`] dropped at this point and [`Cond::signal()`] happens on drop.
        // Another words, [`Cond::signal()`] happens anyway, regardless of the existence of message in the channel.
        // After that, the receiver interprets the lack of a message as a disconnect.
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let cond = Arc::clone(&self.channel.cond);
        let msg = Message::new(move || {
            cond.signal();
        });
        self.pipe.push_message(msg);
    }
}

impl<T> EndpointReceiver<T> {
    /// Attempts to wait for a value on this receiver, returns a [`RecvError`]
    /// if the corresponding channel has hung up (sender was dropped).
    pub fn receive(self) -> Result<T, RecvError> {
        if !self.channel.ready.swap(false, Ordering::Acquire) {
            // assume that situation when [`crate::fiber::Cond::signal()`] called before
            // [`crate::fiber::Cond::wait()`] and after swap `ready` to false  is never been happen,
            // cause signal and wait both calling in tx thread (or any other cord) and there is now yields between it
            self.channel.cond.wait();
        }
        unsafe {
            self.channel
                .message
                .get()
                .as_mut()
                .expect("unexpected null pointer")
                .take()
        }
        .ok_or(RecvError::Disconnected)
    }
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "internal_test")]
mod tests {
    use super::super::tests::run_cbus_endpoint;
    use crate::cbus;
    use crate::cbus::{oneshot, RecvError};
    use crate::fiber::{check_yield, YieldResult};
    use std::sync::Arc;
    use std::time::Duration;
    use std::{mem, thread};

    #[crate::test(tarantool = "crate")]
    pub fn oneshot_test() {
        let mut cbus_fiber = run_cbus_endpoint("oneshot_test");

        let (sender, receiver) = oneshot::channel("oneshot_test");
        let thread = thread::spawn(move || {
            thread::sleep(Duration::from_secs(1));
            sender.send(1);
        });

        assert_eq!(
            check_yield(|| { receiver.receive().unwrap() }),
            YieldResult::Yielded(1)
        );
        thread.join().unwrap();

        let (sender, receiver) = oneshot::channel("oneshot_test");
        let thread = thread::spawn(move || {
            sender.send(2);
        });
        thread.join().unwrap();

        assert_eq!(
            check_yield(|| { receiver.receive().unwrap() }),
            YieldResult::DidntYield(2)
        );

        cbus_fiber.cancel();
    }

    #[crate::test(tarantool = "crate")]
    pub fn oneshot_multiple_channels_test() {
        let mut cbus_fiber = run_cbus_endpoint("oneshot_multiple_channels_test");

        let pipe = cbus::LCPipe::new("oneshot_multiple_channels_test");
        let pipe = Arc::new(pipe);

        let (sender1, receiver1) = oneshot::channel_on_pipe(Arc::clone(&pipe));
        let (sender2, receiver2) = oneshot::channel_on_pipe(Arc::clone(&pipe));

        let thread1 = thread::spawn(move || {
            thread::sleep(Duration::from_secs(1));
            sender1.send("1");
        });

        let thread2 = thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            sender2.send("2");
        });

        let result2 = receiver2.receive();
        let result1 = receiver1.receive();

        assert!(matches!(result1, Ok("1")));
        assert!(matches!(result2, Ok("2")));

        thread1.join().unwrap();
        thread2.join().unwrap();
        cbus_fiber.cancel();
    }

    #[crate::test(tarantool = "crate")]
    pub fn oneshot_sender_drop_test() {
        let mut cbus_fiber = run_cbus_endpoint("oneshot_sender_drop_test");

        let (sender, receiver) = oneshot::channel::<()>("oneshot_sender_drop_test");

        let thread = thread::spawn(move || {
            thread::sleep(Duration::from_secs(1));
            mem::drop(sender)
        });

        let result = receiver.receive();
        assert!(matches!(result, Err(RecvError::Disconnected)));

        thread.join().unwrap();
        cbus_fiber.cancel();
    }
}
