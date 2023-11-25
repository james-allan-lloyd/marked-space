use std::env;

use dotenvy::dotenv;
use serde::Deserialize;

struct ConfluenceClient {
    client: reqwest::blocking::Client,
    api_user: String,
    api_token: String,
    hostname: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Space {
    id: String,
    key: String,
    name: String,
    homepage_id: String,
}

impl ConfluenceClient {
    fn new(hostname: &str) -> ConfluenceClient {
        ConfluenceClient {
            api_user: env::var("API_USER").expect("API_USER not set"),
            api_token: env::var("API_TOKEN").expect("API_TOKEN not set"),
            client: reqwest::blocking::Client::new(),
            hostname: String::from(hostname),
        }
    }

    fn get_space_id(
        &self,
        space_key: &str,
    ) -> std::result::Result<reqwest::blocking::Response, reqwest::Error> {
        let url = format!("https://{}/wiki/api/v2/spaces", self.hostname);
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .query(&[("keys", space_key)])
            .send()
    }

    fn get_space(
        &self,
        space_id: &str,
    ) -> std::result::Result<reqwest::blocking::Response, reqwest::Error> {
        let url = format!("https://{}/wiki/api/v2/spaces/{}", self.hostname, space_id);
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .send()
    }
}

#[derive(Debug, Clone)]
struct ConfluenceError {
    message: String,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn get_homepage(host: &str, space_id: &str) -> Result<String> {
    let confluence_client = ConfluenceClient::new(host);

    let resp = confluence_client.get_space_id(space_id).and_then(|resp| {
        if resp.status().is_success() {
            Ok(resp)
        } else {
            Err(ConfluenceError {
                message: "Failed to get space id",
            })
        }
    });
    if let Err(err) = resp {
        return Err(err);
    } else if let Ok(resp) = resp {
        if resp.status().is_success() {
            let json = resp.json::<serde_json::Value>()?;

            println!("{:#?}", json);

            let space = serde_json::from_value::<Space>(json["results"][0].clone());
            println!("{:#?}", space);
        }
    }

    return Ok(String::from("foo"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().expect(".env file not found");

    match (env::var("API_USER"), env::var("API_TOKEN")) {
        (Err(_), Err(_)) => {
            return Err(Box::from("Missing API_USER and API_TOKEN"));
        }
        (Err(_), Ok(_)) => {
            return Err(Box::from("Missing API_USER"));
        }
        (Ok(_), Err(_)) => {
            return Err(Box::from("Missing API_TOKEN"));
        }
        (Ok(_), Ok(_)) => (),
    }

    get_homepage("jimjim256.atlassian.net", "TEAM");

    // let url = "https://jimjim256.atlassian.net/wiki/api/v2/pages";

    // auth = HTTPBasicAuth("email@example.com", "<api_token>")
    // headers = {
    // "Accept": "application/json",
    // "Content-Type": "application/json"
    // }

    // payload = json.dumps( {
    // "spaceId": "<string>",
    // "status": "current",
    // "title": "<string>",
    // "parentId": "<string>",
    // "body": {
    //     "representation": "storage",
    //     "value": "<string>"
    // }
    // } )
    // let payload = json!({
    //     "spaceId": "TEAM",
    //     "status": "current",
    //     "title": "Test Page",
    //     "parentId": "<string>",
    //     "body": {
    //         "representation": "storage",
    //         "value": "<string>"
    //     }
    // });

    // response = requests.request(
    // "POST",
    // url,
    // data=payload,
    // headers=headers,
    // auth=auth
    // )

    // let client = reqwest::Client::new();

    // let resp = client
    //     .post("https://httpbin.org/ip")
    //     .json::<HashMap<String, String>>()
    //     .send();
    // println!("{:#?}", resp);
    Ok(())
}
