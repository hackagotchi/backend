#[actix_rt::test]
async fn test_get_hackstead() -> hcor::ClientResult<()> {
    use hcor::Hackstead;

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    // create bob's stead!
    let bobstead = Hackstead::register().await?;

    // fetch bob
    assert_eq!(Hackstead::fetch(&bobstead).await?, bobstead);

    // now kill bob
    bobstead.slaughter().await?;

    // make sure we can't get ded bob
    match Hackstead::fetch(&bobstead).await {
        Err(e) => log::info!("fetching ded bobstead failed as expected: {}", e),
        Ok(_) => panic!("fetching bobstead succeeded after he was slaughtered!"),
    }

    // make sure we can't kill an already dead bob
    match bobstead.slaughter().await {
        Err(e) => log::info!("received error as expected killing bob a second time: {}", e),
        Ok(_) => panic!("killing bob a second time worked somehow"),
    }

    Ok(())
}
