# Rust UML Parser
[![Crates.io - uml_parser](https://img.shields.io/crates/v/uml_parser.svg)](https://crates.io/crates/uml_parser) [![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

This is a rust UML parser library for tokenizing UML written compatible with PlantUML (http://plantuml.com/).

## Example of use
Below is an example of using the UML parser library:

```
let uml = parse_uml_file(file.to_str().unwrap(), None);
```

## Contributing
Please see CONTRIBUTING.md for details on how to contribute to the project.
