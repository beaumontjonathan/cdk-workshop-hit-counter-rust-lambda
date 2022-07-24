use aws_sdk_dynamodb::{model::AttributeValue, Client as DynamoDbClient};
use aws_sdk_lambda::{output::InvokeOutput, Client as LambdaClient};
use aws_smithy_types::Blob;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde::Deserialize;
use serde_json::Value;

async fn increment_and_invoke(
    event: Event,
    event_string: String,
    dynamodb_client: &DynamoDbClient,
    lambda_client: &LambdaClient,
) -> InvokeOutput {
    let table_name = std::env::var("HITS_TABLE_NAME").unwrap();
    let downstream_function_name = std::env::var("DOWNSTREAM_FUNCTION_NAME").unwrap();

    println!("Incrementing hit count for path: \"{}\"", &event.path);

    dynamodb_client
        .update_item()
        .table_name(table_name)
        .key("path", AttributeValue::S(event.path))
        .update_expression("ADD hits :incr")
        .expression_attribute_values(":incr", AttributeValue::N("1".to_owned()))
        .send()
        .await
        .unwrap();

    println!("Invoking lambda");

    let response = lambda_client
        .invoke()
        .function_name(downstream_function_name)
        .payload(Blob::new(event_string))
        .send()
        .await
        .unwrap();

    response
}

#[derive(Debug, Deserialize)]
struct Event {
    path: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let sdk_config = aws_config::load_from_env().await;

    let dynamodb_client = DynamoDbClient::new(&sdk_config);
    let lambda_client = LambdaClient::new(&sdk_config);

    let dynamodb_client_ref = &dynamodb_client;
    let lambda_client_ref = &lambda_client;

    lambda_runtime::run(service_fn(move |event: LambdaEvent<Value>| async move {
        func(event, dynamodb_client_ref, lambda_client_ref).await
    }))
    .await?;

    Ok(())
}

async fn func(
    event_raw: LambdaEvent<Value>,
    dynamodb_client: &DynamoDbClient,
    lambda_client: &LambdaClient,
) -> Result<Value, Error> {
    let (event_value, _context) = event_raw.into_parts();
    let event_string = serde_json::to_string(&event_value).unwrap();
    let event: Event = serde_json::from_str(&event_string).unwrap();
    let invoke_output =
        increment_and_invoke(event, event_string, dynamodb_client, lambda_client).await;
    let value =
        serde_json::from_slice::<Value>(&invoke_output.payload.unwrap().into_inner()).unwrap();
    Ok(value)
}
