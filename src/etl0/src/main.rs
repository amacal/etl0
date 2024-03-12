mod docker;
mod pipeline;

use chrono::Utc;
use std::env::current_dir;

use tokio;
use tokio_stream::StreamExt;

use self::docker::{ContainerCreate, ContainerCreateResponse, ContainerLogs, DockerClient, ImageCreate};
use self::pipeline::find_pipelines;

#[tokio::main]
async fn main() {
    for pipeline in find_pipelines(current_dir().unwrap()).await {
        for task in pipeline.tasks() {
            println!("{:?}", task);
            task.execute().await;
        }
    }

    let socket = "/var/run/docker.sock";
    let engine: DockerClient = DockerClient::open(socket).await;

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

    let container: ContainerCreateResponse = match engine.containers_create().await {
        Err(error) => return println!("{:?}", error),
        Ok(ContainerCreate::Succeeded(response)) => response,
        Ok(value) => return println!("{:?}", value),
    };

    println!("{:?}", engine.containers_list().await);
    println!("{:?}", engine.containers_start(&container.id).await);
    println!("{:?}", engine.containers_wait(&container.id).await);

    match engine.containers_logs(&container.id).await {
        Ok(ContainerLogs::Succeeded(mut stream)) => {
            while let Some(item) = stream.next().await {
                println!("{} {:?}", Utc::now().timestamp_millis(), item);
            }
        }
        Err(error) => println!("{:?}", error),
        Ok(value) => println!("{:?}", value),
    }

    println!("{:?}", engine.containers_stop(&container.id).await);
    println!("{:?}", engine.containers_remove(&container.id).await);
}
