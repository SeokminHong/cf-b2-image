use std::path::Path;

use aws_sdk_s3::types::ByteStream;
use aws_sdk_s3::{Client, Error};

pub async fn download_object(client: &Client, bucket_name: &str, key: &str) -> Result<(), Error> {
    let resp = client
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await?;
    let data = resp.body.collect().await;
    println!(
        "Data from downloaded object: {:?}",
        data.unwrap().into_bytes().slice(0..20)
    );

    Ok(())
}

pub async fn upload_object(
    client: &Client,
    bucket_name: &str,
    file_name: &str,
    key: &str,
) -> Result<(), Error> {
    let body = ByteStream::from_path(Path::new(file_name)).await;
    client
        .put_object()
        .bucket(bucket_name)
        .key(key)
        .body(body.unwrap())
        .send()
        .await?;

    println!("Uploaded file: {}", file_name);
    Ok(())
}
