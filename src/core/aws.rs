use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{config::Region, Client};
use aws_types::SdkConfig;
use log::log_enabled;

pub async fn aws_config(region: Option<String>) -> SdkConfig {
    let region_provider = RegionProviderChain::first_try(region.map(Region::new))
        .or_default_provider()
        .or_else(Region::new("ca-central-1"));

    if log_enabled!(log::Level::Trace) {
        let region = region_provider.region().await.unwrap();
        log::trace!("Using S3 region {region}")
    }

    aws_config::from_env().region(region_provider).load().await
}

pub async fn aws_client(config: &SdkConfig) -> Client {
    Client::new(config)
}
