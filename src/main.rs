use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use verified_marketplace_handoff::infrai_client::InfraiClient;
use verified_marketplace_handoff::marketplace_handoff::{execute_handoff, HandoffRequest};

fn main() {
    if let Err(error) = block_on(run()) {
        eprintln!("handoff rejected: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let request = HandoffRequest {
        company_domain: next_arg(&mut args, "company-domain")?,
        employee_email: next_arg(&mut args, "employee-email")?,
        workspace: next_arg(&mut args, "workspace")?,
        seller_asset: next_arg(&mut args, "seller-asset")?,
        buyer_update: next_arg(&mut args, "buyer-update")?,
        order_id: next_arg(&mut args, "order-id")?,
    };
    let receipt = execute_handoff(&InfraiClient::from_env()?, request).await?;
    println!(
        "joined workspace={} user_id={} order_id={}",
        receipt.workspace, receipt.user_id, receipt.order_id
    );
    Ok(())
}

fn next_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing argument: {name}"))
}

fn block_on<F: Future>(mut future: F) -> F::Output {
    fn clone(_: *const ()) -> RawWaker {
        raw_waker()
    }
    fn no_op(_: *const ()) {}
    fn raw_waker() -> RawWaker {
        RawWaker::new(
            std::ptr::null(),
            &RawWakerVTable::new(clone, no_op, no_op, no_op),
        )
    }
    let waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut context = Context::from_waker(&waker);
    let mut future = unsafe { Pin::new_unchecked(&mut future) };
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}
