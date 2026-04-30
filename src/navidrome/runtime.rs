use std::future::Future;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RT: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RT.get_or_init(|| {
        Runtime::new().expect("failed to build tokio runtime for Navidrome / Subsonic API")
    })
}

pub fn block_on<F: Future>(f: F) -> F::Output {
    runtime().block_on(f)
}
