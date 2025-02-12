use dionysus::oracles::{backtrace, MeanReversionOracle};
use dionysus::time::{Duration, Period};

#[tokio::main]
async fn main() {
    let mean_reversion = MeanReversionOracle {};
    backtrace("AAPL", Period::last(Duration::days(31)), &mean_reversion).await;
}
