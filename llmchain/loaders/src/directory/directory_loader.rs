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

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use glob_match::glob_match;
use opendal::EntryMode;
use opendal::Metakey;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use rayon::ThreadPoolBuilder;

use crate::Disk;
use crate::Document;
use crate::DocumentLoader;
use crate::DocumentPath;

pub struct DirectoryLoader {
    disk: Arc<dyn Disk>,
    loaders: HashMap<String, Arc<dyn DocumentLoader>>,
    max_threads: usize,
}

impl DirectoryLoader {
    pub fn create(disk: Arc<dyn Disk>) -> Self {
        DirectoryLoader {
            disk,
            loaders: HashMap::default(),
            max_threads: 8,
        }
    }

    pub fn with_loader(mut self, glob: &str, loader: Arc<dyn DocumentLoader>) -> Self {
        self.loaders.insert(glob.to_string(), loader);
        self
    }

    pub fn with_max_threads(mut self, max_threads: usize) -> Self {
        self.max_threads = max_threads;
        self
    }

    fn process_directory(
        &self,
        path: &str,
        tasks: &mut Vec<(String, Arc<dyn DocumentLoader>)>,
    ) -> Result<()> {
        let op = self.disk.get_operator()?;
        let ds = op.scan(path)?;
        for de in ds {
            let de = de?;
            let path_buf = de.path();
            let path_str = path_buf.to_string();
            let meta = op.metadata(&de, Metakey::Mode)?;
            match meta.mode() {
                EntryMode::FILE => {
                    for loader in &self.loaders {
                        if glob_match(loader.0, &path_str) {
                            tasks.push((path_str, loader.1.clone()));
                            break;
                        }
                    }
                }
                _ => continue,
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl DocumentLoader for DirectoryLoader {
    async fn load(&self, path: DocumentPath) -> Result<Vec<Document>> {
        let mut tasks: Vec<(String, Arc<dyn DocumentLoader>)> = Vec::new();
        self.process_directory(path.as_str()?, &mut tasks)?;

        let worker_pool = ThreadPoolBuilder::new()
            .num_threads(self.max_threads)
            .build()?;
        let results: Vec<_> = worker_pool.install(|| {
            tasks
                .par_iter()
                .map(|(path, loader)| loader.load(DocumentPath::from_string(path)))
                .collect()
        });

        let mut documents = vec![];
        for result in results {
            let result = result.await?;
            documents.extend(result);
        }

        Ok(documents)
    }
}
