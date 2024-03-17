mod docker;
mod pipeline;
mod tar;

use std::io::Write;
use chrono::Utc;

use tar::TarChunk;
use tokio;
use tokio_stream::StreamExt;

use crate::docker::{ContainerAttach, ContainerCreateSpec, ContainerList};
use crate::docker::{ContainerCreate, ContainerCreateResponse, DockerClient, ImageCreate};
use crate::tar::TarArchive;

async fn archive_test() {
    let mut archive = TarArchive::new();
    archive.append_file("enwiki-20230801-pages-meta-history27.xml-p74198591p74500204".to_owned());
    archive.append_file("lubuntu-22.04.3-desktop-amd64.iso".to_owned());
    archive.append_file("qemu-8.2.1.tar.xz".to_owned());

    let mut stream = archive.into_stream(10 * 1024 * 1024);

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(TarChunk::Header(path, _)) => println!("\nheader {path}"),
            Ok(TarChunk::Data(_)) => print!("."),
            Ok(TarChunk::Padding(0)) => println!("\npadding 0"),
            Ok(TarChunk::Padding(index)) => println!("padding {index}"),
            Err(error) => println!("error: {:?}", error),
        }

        std::io::stdout().flush().unwrap();
    }
}

#[tokio::main]
async fn main() {
    return archive_test().await;

    let socket = "/var/run/docker.sock";
    let engine: DockerClient = DockerClient::open(socket);

    match engine.images_create().await {
        Err(error) => return println!("{:?}", error),
        Ok(value) => match value {
            ImageCreate::Succeeded(mut stream) => {
                while let Some(item) = stream.next().await {
                    println!("{} {:?}", Utc::now().timestamp_millis(), item);
                }
            }
            value => println!("{:?}", value),
        },
    }

    let spec = ContainerCreateSpec {
        image: "ubuntu:latest",
        command: vec![
            "sha256sum",
            "/opt/lubuntu-22.04.3-desktop-amd64.iso",
            "/opt/enwiki-20230801-pages-meta-history27.xml-p74198591p74500204",
            "/opt/qemu-8.2.1.tar.xz",
        ],
    };

    let container: ContainerCreateResponse = match engine.containers_create(&spec).await {
        Err(error) => return println!("{:?}", error),
        Ok(ContainerCreate::Succeeded(response)) => response,
        Ok(value) => return println!("{:?}", value),
    };

    let mut archive = TarArchive::new();
    archive.append_file("enwiki-20230801-pages-meta-history27.xml-p74198591p74500204".to_owned());
    archive.append_file("lubuntu-22.04.3-desktop-amd64.iso".to_owned());
    archive.append_file("qemu-8.2.1.tar.xz".to_owned());

    println!("{:?}", engine.container_upload(&container.id, "/opt", archive).await);

    let mut stream = match engine.containers_attach(&container.id).await {
        Ok(ContainerAttach::Succeeded(stream)) => stream,
        Err(error) => return println!("{:?}", error),
        Ok(value) => return println!("{:?}", value),
    };

    println!("{:?}", engine.containers_start(&container.id).await);
    while let Some(item) = stream.next().await {
        println!("{} {:?}", Utc::now().timestamp_millis(), item);
    }

    println!("{:?}", engine.containers_wait(&container.id).await);
    println!("{:?}", engine.containers_stop(&container.id).await);

    match engine.containers_list().await {
        Err(error) => println!("{}", error),
        Ok(ContainerList::BadParameter(value)) => println!("{:?}", value),
        Ok(ContainerList::ServerError(value)) => println!("{:?}", value),
        Ok(ContainerList::Succeeded(containers)) => {
            for container in containers {
                //println!(
                //    "{} | {:>32} | {}",
                //    &container.id[0..8],
                //    container.status,
                //    container.image
                //);

                if container.image == spec.image {
                    println!("{:?}", engine.containers_remove(&container.id).await);
                }
            }
        }
    }
}
