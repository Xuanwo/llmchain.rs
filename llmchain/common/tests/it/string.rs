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

use llmchain_common::escape_sql_string;

#[test]
fn test_escape_sql_string() {
    let input = "Hello, World!";
    let output = escape_sql_string(input);
    assert_eq!(output, "Hello, World!");

    let input = "Hello, 'World'!";
    let output = escape_sql_string(input);
    assert_eq!(output, "Hello, ''World''!");

    let input = "Hello, 'World'! \n";
    let output = escape_sql_string(input);
    assert_eq!(output, "Hello, ''World''!  ");

    let input = "Hello, 'World'! \r";
    let output = escape_sql_string(input);
    assert_eq!(output, "Hello, ''World''! \\r");
}
