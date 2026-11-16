//! The mail folder tree. Delta is per folder, so the engine needs every folder id, children included.
use contracts::CoreError;
use reqwest::Method;
use serde::Deserialize;

use crate::{
    client::GraphMailbox,
    model::{invalid, parse},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphFolder {
    pub id: String,
    pub display_name: String,
    pub parent_id: Option<String>,
}

#[derive(Deserialize)]
struct FolderPage {
    #[serde(default)]
    value: Vec<RawFolder>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFolder {
    id: String,
    display_name: String,
    parent_folder_id: Option<String>,
    #[serde(default)]
    child_folder_count: u32,
}

impl GraphMailbox {
    async fn folder_pages(&self, first: String) -> Result<Vec<RawFolder>, CoreError> {
        let headers = self.page_header();
        let mut out = Vec::new();
        let mut url = first;
        let mut seen = std::collections::HashSet::new();
        loop {
            if !seen.insert(url.clone()) {
                return Err(invalid("@odata.nextLink", "repeats a link already followed"));
            }
            self.check_origin(&url)?;
            let page: FolderPage = parse(&self.send(Method::GET, &url, &headers, None, false).await?.body, "folder page")?;
            out.extend(page.value);
            match page.next_link {
                Some(n) => url = n,
                None => return Ok(out),
            }
        }
    }

    /// Top-level folders, then the children of every folder that has any, depth first.
    pub async fn list_folders(&self) -> Result<Vec<GraphFolder>, CoreError> {
        let mut out = Vec::new();
        let mut todo = vec![self.mailbox_url(&["mailFolders"])?.to_string()];
        while let Some(url) = todo.pop() {
            for f in self.folder_pages(url).await? {
                if f.child_folder_count > 0 {
                    todo.push(self.mailbox_url(&["mailFolders", &f.id, "childFolders"])?.to_string());
                }
                out.push(GraphFolder { id: f.id, display_name: f.display_name, parent_id: f.parent_folder_id });
            }
        }
        Ok(out)
    }
}
