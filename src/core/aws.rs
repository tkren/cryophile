// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use aws_config::{meta::region::RegionProviderChain, BehaviorVersion};
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

    aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await
}

pub async fn aws_client(config: &SdkConfig) -> Client {
    Client::new(config)
}
