use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Error;

mod options;

pub use options::{TaskOptions, TaskSendOptions, TaskSendOptionsBuilder};

/// A `Task` represents a unit of work that a `Celery` app can produce or consume.
///
/// The recommended way to define a task is through the [`task`](attr.task.html) procedural macro:
///
/// ```rust
/// use celery::task;
///
/// #[task(name = "add")]
/// fn add(x: i32, y: i32) -> i32 {
///     x + y
/// }
/// ```
///
/// However, if you need more fine-grained control, it is fairly straight forward
/// to implement a task directly. This would be equivalent to above:
///
/// ```rust
/// use async_trait::async_trait;
/// use serde::{Serialize, Deserialize};
/// use celery::{Task, Error};
///
/// #[allow(non_camel_case_types)]
/// #[derive(Serialize, Deserialize)]
/// struct add {
///     x: i32,
///     y: i32,
/// }
///
/// #[async_trait]
/// impl Task for add {
///     const NAME: &'static str = "add";
///     const ARGS: &'static [&'static str] = &["x", "y"];
///
///     type Returns = i32;
///
///     async fn run(mut self) -> Result<Self::Returns, Error> {
///         Ok(self.x + self.y)
///     }
/// }
///
/// impl add {
///     fn new(x: i32, y: i32) -> Self {
///         Self { x, y }
///     }
/// }
/// ```
///
/// Within the [Celery protocol](https://docs.celeryproject.org/en/latest/internals/protocol.html#version-2)
/// the task parameters can be treated as either `args` (positional) or `kwargs` (key-word based).
/// So, for example, from Python the `add` task could be called like
///
/// ```python
/// celery_app.send_task("add", args=[1, 2])
/// ```
///
/// or
///
/// ```python
/// celery_app.send_task("add", kwargs={"x": 1, "y": 2})
/// ```
///
/// # Making task parameters optional
///
/// You can provide default values for task parameters through the
/// [deserialization mechanism](https://serde.rs/attr-default.html). Currently this means
/// you'll have to implement the task manually as opposed to using the `#[task]` macro.
///
/// So if we wanted to make the `y` parameter in the `add` task optional with a default
/// value of 0, we could add the `#[serde(default)]` macro to the `y` field:
///
/// ```rust
/// # use async_trait::async_trait;
/// # use serde::{Serialize, Deserialize};
/// # use celery::{Task, Error};
/// #[allow(non_camel_case_types)]
/// #[derive(Serialize, Deserialize)]
/// struct add {
///     x: i32,
///     #[serde(default)]
///     y: i32,
/// }
/// # #[async_trait]
/// # impl Task for add {
/// #     const NAME: &'static str = "add";
/// #     const ARGS: &'static [&'static str] = &["x", "y"];
/// #     type Returns = i32;
/// #     async fn run(mut self) -> Result<Self::Returns, Error> {
/// #         Ok(self.x + self.y)
/// #     }
/// # }
/// ```
///
/// # Error handling
///
/// As demonstrated above, the `#[task]` macro tries wrapping the return value in `Result<Self::Returns, Error>`
/// when it is ran. Therefore the recommended way to propogate errors when defining a task with
/// `#[task]` is to use `.context("...")?` on `Result` types within the task body.
///
/// For example:
///
/// ```rust
/// use celery::{task, ResultExt};
///
/// #[task(name = "add")]
/// fn read_some_file() -> String {
///     tokio::fs::read_to_string("some_file")
///         .await
///         .context("File does not exist")?
/// }
/// ```
///
/// The `.context` method on a `Result` comes from the [`ResultExt`](trait.ResultExt.html) trait.
/// This is used to provide additional human-readable context to the error and also
/// to convert it into the expected [`Error`](struct.Error.html) type.
#[async_trait]
pub trait Task: Send + Sync + Serialize + for<'de> Deserialize<'de> {
    /// The name of the task. When a task is registered it will be registered with this name.
    const NAME: &'static str;

    /// For compatability with Python tasks. This keeps track of the order
    /// of arguments for the task so that the task can be called from Python with
    /// positional arguments.
    const ARGS: &'static [&'static str];

    /// The return type of the task.
    type Returns: Send + Sync + std::fmt::Debug;

    /// This function defines how a task executes.
    async fn run(mut self) -> Result<Self::Returns, Error>;

    /// Callback that will run after a task fails.
    /// It takes a reference to a `TaskContext` struct and the error returned from task.
    #[allow(unused_variables)]
    async fn on_failure(ctx: &TaskContext<'_>, err: &Error) {}

    /// Callback that will run after a task completes successfully.
    /// It takes a reference to a `TaskContext` struct and the returned value from task.
    #[allow(unused_variables)]
    async fn on_success(ctx: &TaskContext<'_>, returned: &Self::Returns) {}

    /// Default timeout for this task.
    fn timeout(&self) -> Option<u32> {
        None
    }

    /// Default maximum number of retries for this task.
    fn max_retries(&self) -> Option<u32> {
        None
    }

    /// Default minimum retry delay (in seconds) for this task (default is 0).
    fn min_retry_delay(&self) -> Option<u32> {
        None
    }

    /// Default maximum retry delay (in seconds) for this task.
    fn max_retry_delay(&self) -> Option<u32> {
        None
    }
}

/// Additional context sent to the `on_success` and `on_failure` task callbacks.
pub struct TaskContext<'a> {
    /// The correlation ID of the task.
    pub correlation_id: &'a str,
}

#[derive(Clone, Debug)]
pub(crate) enum TaskStatus {
    Pending,
    Finished,
}

#[derive(Clone, Debug)]
pub(crate) struct TaskEvent {
    pub(crate) status: TaskStatus,
}

impl TaskEvent {
    pub(crate) fn new(status: TaskStatus) -> Self {
        Self { status }
    }
}
