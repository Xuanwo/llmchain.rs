// Copyright 2023 Shafish Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::Arc;

use anyhow::Result;
use databend_driver::new_connection;
use futures_util::StreamExt;
use llmchain_common::escape_sql_string;
use llmchain_embeddings::Embedding;
use llmchain_loaders::Document;
use uuid::Uuid;

use crate::VectorStore;

pub struct DatabendVectorStore {
    dsn: String,
    database: String,
    table: String,
    embedding: Arc<dyn Embedding>,
    min_similarity: f32,
}

impl DatabendVectorStore {
    pub fn create(dsn: &str, embedding: Arc<dyn Embedding>) -> Self {
        DatabendVectorStore {
            dsn: dsn.to_string(),
            database: "embedding_store".to_string(),
            table: "llmchain_collection".to_string(),
            embedding,
            min_similarity: 0.5,
        }
    }

    pub fn with_database(mut self, database: &str) -> Self {
        self.database = database.to_string();
        self
    }

    pub fn with_table(mut self, table: &str) -> Self {
        self.table = table.to_string();
        self
    }

    pub fn with_min_similarity(mut self, similarity: f32) -> Self {
        self.min_similarity = similarity;
        self
    }
}

#[async_trait::async_trait]
impl VectorStore for DatabendVectorStore {
    async fn init(&self) -> Result<()> {
        let conn = new_connection(&self.dsn)?;

        let database_create_sql = format!("CREATE DATABASE IF NOT EXISTS {}", self.database);
        conn.exec(&database_create_sql).await?;

        let table_create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {}.{} \
            (uuid VARCHAR, path VARCHAR, content VARCHAR, content_md5 VARCHAR, embedding ARRAY(float32))",
            self.database, self.table
        );
        conn.exec(&table_create_sql).await?;

        Ok(())
    }

    async fn add_documents(&self, inputs: Vec<Document>) -> Result<Vec<String>> {
        let uuids = (0..inputs.len())
            .map(|_| Uuid::new_v4().to_string())
            .collect::<Vec<_>>();

        let embeddings = self.embedding.embed_documents(inputs.clone()).await?;

        let sql = format!(
            "INSERT INTO {}.{} (uuid, path, content, content_md5, embedding) VALUES ",
            self.database, self.table
        );
        let mut val_vec = vec![];
        for (idx, doc) in inputs.iter().enumerate() {
            val_vec.push(format!(
                "('{}', '{}', '{}', '{}', {:?})",
                uuids[idx],
                escape_sql_string(&doc.path),
                escape_sql_string(&doc.content),
                doc.content_md5,
                embeddings[idx]
            ));
        }
        let values = val_vec.join(",").to_string();

        let final_sql = format!("{} {}", sql, values);
        let conn = new_connection(&self.dsn)?;
        conn.exec(&final_sql).await?;

        Ok(uuids)
    }

    async fn similarity_search(&self, query: &str, k: usize) -> Result<Vec<Document>> {
        let query_embedding = self.embedding.embed_query(query).await?;

        let sql = format!(
            "SELECT path, content, content_md5, (1- cosine_distance({:?}, embedding)) AS similarity FROM {}.{} \
             WHERE length(embedding) > 0 AND length(content) > 0 AND similarity > {} ORDER BY similarity DESC LIMIT {}",
            query_embedding, self.database, self.table, self.min_similarity, k
        );

        let mut documents = vec![];
        type RowResult = (String, String, String, f32);
        let conn = new_connection(&self.dsn)?;
        let mut rows = conn.query_iter(&sql).await?;
        while let Some(row) = rows.next().await {
            let row: RowResult = row?.try_into()?;
            documents.push(Document {
                path: row.0,
                content: row.1,
                content_md5: row.2,
            });
        }

        Ok(documents)
    }
}
