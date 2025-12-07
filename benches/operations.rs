use bip39::Mnemonic;
use cdk::Amount;
use cdk::amount::SplitTarget;
use cdk::nuts::CurrencyUnit;
use cdk::wallet::Wallet;
use cdk_sqlite::wallet::memory;
use futures::future;
use hdrhistogram::Histogram;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MINT_URL: &str = "http://127.0.0.1:8086";
const CDK_MINT: bool = true; // true for CDK mint, false for JCMint

fn print_histogram(hist: &Histogram<u64>, label: &str) {
    println!("--- {} Benchmark Results ---", label);
    println!("Mean:   {:.2} us", hist.mean());
    println!("StdDev: {:.2} us", hist.stdev());
    println!("Min:    {} us", hist.min());
    println!("Max:    {} us", hist.max());
    println!("50%:    {} us", hist.value_at_percentile(50.0));
    println!("90%:    {} us", hist.value_at_percentile(90.0));
    println!("99%:    {} us", hist.value_at_percentile(99.0));
    println!("99.9%:  {} us", hist.value_at_percentile(99.9));
    println!("n={}", hist.len());
    println!()
}

#[tokio::main]
async fn main() {
    // Setup wallet
    let wallet = Wallet::new(
        MINT_URL,
        CurrencyUnit::Sat,
        Arc::new(memory::empty().await.unwrap()),
        Mnemonic::generate(12).unwrap().to_seed_normalized(""),
        None,
    )
    .expect("failed to create wallet");

    // pre-generate quotes - not measured
    let mut pending_quotes = Vec::new();
    for _ in 0..100 {
        let amount = Amount::from(1);
        let quote = wallet.mint_quote(amount, None).await.unwrap();
        pending_quotes.push(quote);
    }
    let quotes = if CDK_MINT {
        let quote_futures = pending_quotes.into_iter().map(|quote| async {
            wallet
                .wait_for_payment(&quote, Duration::from_secs(10))
                .await
                .unwrap();
            quote
        });
        future::join_all(quote_futures).await
    } else {
        pending_quotes
    };

    // Benchmark mint operation (n=100)
    let mut hist_mint: Histogram<u64> = Histogram::new(3).unwrap();
    for quote in quotes {
        let start = Instant::now();

        // Mint
        let _minted_proofs = wallet
            .mint(&quote.id, SplitTarget::default(), None)
            .await
            .unwrap();

        let elapsed = start.elapsed();
        hist_mint.record(elapsed.as_micros() as u64).unwrap();
    }
    print_histogram(&hist_mint, "Mint");

    // Benchmark swap operation (n=100)
    let proofs = wallet.get_unspent_proofs().await.unwrap();
    let mut hist_swap: Histogram<u64> = Histogram::new(3).unwrap();
    for proof in proofs {
        let start = Instant::now();

        // Swap
        let new_proofs = wallet
            .swap(None, SplitTarget::None, vec![proof], None, false)
            .await;

        if new_proofs.is_err() {
            eprintln!("Swap error: {:?}", new_proofs.err());
        } else {
            let elapsed = start.elapsed();
            hist_swap.record(elapsed.as_micros() as u64).unwrap();
        }
    }
    print_histogram(&hist_swap, "Swap");
}
