#![allow(missing_docs)]
#[macro_use]
extern crate nom;
#[macro_use]
extern crate log;

use nom::{digit, line_ending, not_line_ending, space, IResult};
use std::fs::File;
use std::io::Read;

mod uml_print;

/// Tokens that represent each of the elements of UML that are supported.
#[derive(Debug, Clone, PartialEq)]
pub enum UMLToken {
    StartUML,
    EndUML,
    Note { position: String, text: String },
    Parallel { sequences: Vec<UMLTokens> },
    Message {
        from: String,
        to: String,
        text: Option<String>,
        colour: Option<String>,
    },
    Participant {
        long_name: Option<String>,
        short_name: String,
    },
    Activate { name: String },
    Deactivate { name: String },
    Loop { sequence: UMLTokens, count: u8 },
    Include { file: String, sequence: UMLTokens },
    Box { name: String, sequence: UMLTokens },
    Destroy { name: String },
    Delay { text: String },
    Alt { sequences: Vec<UMLTokens> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct UMLTokens {
    pub tokens: Vec<UMLToken>,
}

impl UMLTokens {
    pub fn new(tokens: Vec<UMLToken>) -> UMLTokens {
        UMLTokens { tokens: tokens }
    }
}

fn take_until_or_line_ending<'a>(input: &'a [u8],
                                 tag: &'static str)
                                 -> IResult<&'a [u8], &'a [u8]> {
    let line_ending_res = not_line_ending(input);

    let output = match line_ending_res {
        IResult::Done(_, output) => output,
        _ => return line_ending_res,
    };

    let take_until_res = take_until!(output, tag);
    match take_until_res {
        IResult::Done(_, tag_output) => {
            let count = tag_output.len();
            IResult::Done(&input[count..], tag_output)
        }
        _ => line_ending_res,
    }
}

/// Parse a UML file and return the `UMLTokens` that were parsed.
pub fn parse_uml_file(file: &str, path: Option<&std::path::Path>) -> UMLTokens {

    let old_path = std::env::current_dir().expect("Can't access current directory");

    if let Some(path) = path {
        info!("Setting current directory to {:?}", path.to_str().unwrap());
        std::env::set_current_dir(&path).unwrap();
    }

    let cur_path = std::env::current_dir().unwrap();
    let file_path = if file.starts_with('/') {
        file.to_string()
    } else {
        format!("{}/{}", cur_path.to_str().unwrap(), file)
    };

    let mut f = File::open(file_path).unwrap();
    let mut uml = String::new();
    f.read_to_string(&mut uml).unwrap();

    // Strip out any \r characters from the file to cope with DOS line endings.
    uml = uml.replace("\r", "");

    info!("Parsing {}", file);
    let result = uml_parser(uml.as_bytes());

    let uml_tokens = match result {
        IResult::Done(_, tokens) => tokens,
        _ => panic!("{:?}", result),
    };
    info!("Done parsing {}", file);

    std::env::set_current_dir(&old_path).unwrap();

    uml_tokens
}

/// `take_until_first_tag!(tag, ...) => &[T] -> IResult<&[T], &[T]>`
///
/// Reads up to the first provided tag (preferring earlier tags if there's
/// conflict).
macro_rules! take_until_first_tag (
    ($i:expr, $($substr:expr),+) => ({
        use $crate::nom::InputLength;
        use $crate::nom::FindSubstring;
        use $crate::nom::Slice;

// This enum tracks the current "best answer" over all provided tags.
//
// It has a cryptically short name, since we can't `use P::*` due to a language
// limitation.
        enum P {
            Nothing,
            Found(usize),
            NeedMore(usize),
        }

        let res = P::Nothing;

        $(let res = if $substr.input_len() > $i.input_len() {
            match res {
                P::Nothing => P::NeedMore($substr.input_len()),
                P::NeedMore(old) if old > $substr.input_len() => P::NeedMore($substr.input_len()),
                _ => res,
            }
        } else if let Some(index) = $i.find_substring($substr) {
            match res {
                P::Nothing | P::NeedMore(_) => P::Found(index),
                P::Found(old) if old > index => P::Found(index),
                _ => res
            }
        } else {
            res
        };)*

        match res {
            P::Nothing => $crate::nom::IResult::Error($crate::nom::ErrorKind::TakeUntil),
            P::Found(index) => $crate::nom::IResult::Done($i.slice(index..), $i.slice(0..index)),
            P::NeedMore(size) => $crate::nom::IResult::Incomplete($crate::nom::Needed::Size(size)),
        }
    })
);

named!(startuml<&[u8], UMLToken>,
    chain!(
        space?                            ~
        tag!("@startuml")                 ~
        space?                            ~
        line_ending
        ,
        || UMLToken::StartUML
    )
);

named!(enduml<&[u8], UMLToken>,
    chain!(
        space?                            ~
        tag!("@enduml")                   ~
        space?                            ~
        line_ending
        ,
        || UMLToken::EndUML
    )
);

named!(include_parser<&[u8], UMLToken>,
    chain!(
        space?                            ~
        tag!("!include")                  ~
        space                             ~
        file: map_res!(
            not_line_ending,
            std::str::from_utf8
        )                                 ~
        line_ending
        ,
        || {
            let file = file.trim().trim_matches('\"').to_string();

            UMLToken::Include {
                file: file.clone(),
                sequence: parse_uml_file(&file, None),
            }
        }
    )
);

named!(participant_parser<&[u8], UMLToken>,
    chain!(
        space?                            ~
        alt!(
            tag!("participant") |
            tag!("actor")
        )                                 ~
        space                             ~
        name: map_res!(
            apply!(
                take_until_or_line_ending, " as "
            ),
            std::str::from_utf8
        )                                 ~
        short_name: opt!(
            chain!(
                tag!(" as ")              ~
                space?                    ~
                text: map_res!(
                    not_line_ending,
                    std::str::from_utf8
                )
                ,
                || {
                    text.trim().to_string()
                }
            )
        )                                 ~
        line_ending
        ,
        || {
            UMLToken::Participant {
                long_name: if short_name.is_some() {
                    Some(name.trim().to_string())
                } else {
                    None
                },
                short_name: if short_name.is_some() {
                    short_name.unwrap().trim().to_string()
                } else {
                    name.trim().to_string()
                }
            }
        }
    )
);

named!(note_parser<&[u8], UMLToken>,
    chain!(
        space?                            ~
        tag!("note")                      ~
        position: map_res!(
            not_line_ending,
            std::str::from_utf8
        )                                 ~
        line_ending                       ~
        text: map_res!(
            take_until!("end note"),
            std::str::from_utf8
        )                                 ~
        tag!("end note")                  ~
        space?                            ~
        line_ending?
        ,
        || {
            UMLToken::Note {
                position: position.trim().to_string(),
                text: text.trim().to_string()
            }
        }
    )
);

named!(loop_parser<&[u8], UMLToken>,
    chain!(
        space?                            ~
        tag!("loop")                      ~
        space?                            ~
        count: map_res!(
            digit,
            std::str::from_utf8
        )                                 ~
        space?                            ~
        line_ending                       ~
        sequence: uml_parser              ~
        space?                            ~
        line_ending?                      ~
        tag!("end")                       ~
        not_line_ending                   ~
        line_ending
        ,
        || {
            UMLToken::Loop {
                sequence: sequence,
                count: count.parse::<u8>().unwrap()
            }
        }
    )
);

named!(box_parser<&[u8], UMLToken>,
    chain!(
        space?                            ~
        tag!("box")                       ~
        space?                            ~
        name: map_res!(
            not_line_ending,
            std::str::from_utf8
        )                                 ~
        space?                            ~
        line_ending                       ~
        sequence: uml_parser              ~
        space?                            ~
        line_ending?                      ~
        tag!("end box")                   ~
        not_line_ending                   ~
        line_ending
        ,
        || {
            UMLToken::Box {
                name: name.trim().to_string(),
                sequence: sequence,
            }
        }
    )
);

named!(message_parser<&[u8], UMLToken>,
    chain!(
        space?                           ~
        participant_1: map_res!(
            take_until_first_tag!("->", "<-"),
            std::str::from_utf8
        )                                ~
        direction: map_res!(
            alt!(
                tag!("->") |
                tag!("<-")
            ),
            std::str::from_utf8
        )                                ~
        participant_2: map_res!(
            apply!(
                take_until_or_line_ending, ":"
            ),
            std::str::from_utf8
        )                                ~
        text: opt!(
            chain!(
                tag!(":")                ~
                text: map_res!(
                    not_line_ending,
                    std::str::from_utf8
                )
                ,
                || {
                    text.trim().to_string()
                }
            )
        )                                ~
        line_ending
        ,
        || {
            let (from, to) = match direction {
                "->" => (participant_1, participant_2),
                "<-" => (participant_2, participant_1),
                _ => panic!("Unhandled direction: {}", direction)
            };

            UMLToken::Message {
                from: from.trim().to_string(),
                to: to.trim().to_string(),
                text: text,
                colour: None
            }
        }

    )
);

named!(par_parser<&[u8], UMLToken>,
  chain!(
    space?                                ~
    tag!("par")                           ~
    not_line_ending                       ~
    line_ending                           ~
    uml_array: many1!(
        chain!(
            tokens: uml_parser            ~
            space?                        ~
            line_ending?                  ~
            tag!("else")?                 ~
            line_ending?
            ,
            || {
                tokens
            }
        )
    )                                     ~
    tag!("end")                           ~
    not_line_ending                       ~
    line_ending
    ,
    || {
        UMLToken::Parallel {
            sequences: uml_array
        }
    }
  )
);

named!(alt_parser<&[u8], UMLToken>,
  chain!(
    space?                                ~
    tag!("alt")                           ~
    not_line_ending                       ~
    line_ending                           ~
    uml_array: many1!(
        chain!(
            tokens: uml_parser            ~
            space?                        ~
            line_ending?                  ~
            tag!("else")?                 ~
            line_ending?
            ,
            || {
                tokens
            }
        )
    )                                     ~
    tag!("end")                           ~
    not_line_ending                       ~
    line_ending
    ,
    || {
        UMLToken::Alt {
            sequences: uml_array
        }
    }
  )
);

named!(delay_parser<&[u8], UMLToken>,
    chain!(
        space?                           ~
        tag!("delay")                    ~
        text: map_res!(
            not_line_ending,
            std::str::from_utf8
        )                                ~
        line_ending
        ,
        || {
            UMLToken::Delay {
                text: text.trim().to_string()
            }
        }
    )
);

named!(activate_parser<&[u8], UMLToken>,
    chain!(
        space?                           ~
        tag!("activate")                 ~
        name: map_res!(
            not_line_ending,
            std::str::from_utf8
        )                                ~
        line_ending
        ,
        || {
            UMLToken::Activate {
                name: name.trim().to_string()
            }
        }
    )
);

named!(deactivate_parser<&[u8], UMLToken>,
    chain!(
        space?                           ~
        tag!("deactivate")               ~
        name: map_res!(
            not_line_ending,
            std::str::from_utf8
        )                                ~
        line_ending
        ,
        || {
            UMLToken::Deactivate {
                name: name.trim().to_string()
            }
        }
    )
);

named!(destroy_parser<&[u8], UMLToken>,
    chain!(
        space?                          ~
        tag!("destroy")                 ~
        name: map_res!(
            not_line_ending,
            std::str::from_utf8
        )                                ~
        line_ending
        ,
        || {
            UMLToken::Destroy {
                name: name.trim().to_string()
            }
        }
    )
);

named!(pub uml_parser<&[u8], UMLTokens >,
    chain!(
        tokens: many1!(
            chain!(
                not!(
                    peek!(
                        alt!(
                            tag!("else") |
                            tag!("end")
                        )
                    )
                )                              ~
                space?                         ~
                line_ending?                   ~
                token: alt!(
                    startuml |
                    enduml |
                    include_parser |
                    note_parser |
                    participant_parser |
                    par_parser |
                    alt_parser |
                    delay_parser |
                    activate_parser |
                    deactivate_parser |
                    destroy_parser |
                    box_parser |
                    loop_parser |
                    message_parser
                )
                ,
                || {
                    token
                }
            )
        )
        ,
        || {
            UMLTokens::new(tokens)
        }
    )
);

#[cfg(test)]
mod tests {
    use super::*;
    use nom::IResult::Done;

    #[test]
    fn test_parse_message() {
        let test_uml = "PERSON_A->PERSON_B\n";
        let result = ::message_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Message {
                            from: "PERSON_A".to_string(),
                            to: "PERSON_B".to_string(),
                            text: None,
                            colour: None,
                        }));
    }

    #[test]
    fn test_parse_message_with_description() {
        let test_uml = "PERSON_A->PERSON_B:Test\n";
        let result = ::message_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Message {
                            from: "PERSON_A".to_string(),
                            to: "PERSON_B".to_string(),
                            text: Some("Test".to_string()),
                            colour: None,
                        }));
    }

    #[test]
    fn test_parse_message_with_description_and_spacing() {
        let test_uml = "PERSON_A  ->    PERSON_B:     Test\n";
        let result = ::message_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Message {
                            from: "PERSON_A".to_string(),
                            to: "PERSON_B".to_string(),
                            text: Some("Test".to_string()),
                            colour: None,
                        }));
    }

    #[test]
    fn test_participant_parser() {
        let test_uml = "participant test\n";
        let result = ::participant_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Participant {
                            short_name: "test".to_string(),
                            long_name: None,
                        }));
    }

    #[test]
    fn test_participant_parser_short_name() {
        let test_uml = "participant \"test name\" as hello\n";
        let result = ::participant_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Participant {
                            short_name: "hello".to_string(),
                            long_name: Some("\"test name\"".to_string()),
                        }));
    }

    #[test]
    fn test_tokens_participant_short_name() {
        let test_uml = r#"participant "test name"
participant "test name" as hello
"#;

        let result = ::uml_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLTokens {
                            tokens: vec![UMLToken::Participant {
                                             short_name: "\"test name\"".to_string(),
                                             long_name: None,
                                         },
                                         UMLToken::Participant {
                                             short_name: "hello".to_string(),
                                             long_name: Some("\"test name\"".to_string()),
                                         }],
                        }));
    }

    #[test]
    fn test_tokens_msg_short_long() {
        let test_uml = r#"TESTA->TESTB
TESTB->TESTA: Hello
"#;

        let result = ::uml_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLTokens {
                            tokens: vec![UMLToken::Message {
                                             from: "TESTA".to_string(),
                                             to: "TESTB".to_string(),
                                             text: None,
                                             colour: None,
                                         },
                                         UMLToken::Message {
                                             from: "TESTB".to_string(),
                                             to: "TESTA".to_string(),
                                             text: Some("Hello".to_string()),
                                             colour: None,
                                         }],
                        }));
    }

    #[test]
    fn test_actor_parser() {
        let test_uml = "actor test\n";
        let result = ::participant_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Participant {
                            short_name: "test".to_string(),
                            long_name: None,
                        }));
    }

    #[test]
    fn test_note_parser() {
        let test_uml = "note position\nquick test\nend note\n";
        let result = ::note_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Note {
                            position: "position".to_string(),
                            text: "quick test".to_string(),
                        }));
    }

    #[test]
    fn test_activate_parser() {
        let test_uml = "activate test\n";
        let result = ::activate_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..], UMLToken::Activate { name: "test".to_string() }));
    }

    #[test]
    fn test_deactivate_parser() {
        let test_uml = "deactivate test\n";
        let result = ::deactivate_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..], UMLToken::Deactivate { name: "test".to_string() }));
    }

    #[test]
    fn test_destroy_parser() {
        let test_uml = "destroy test\n";
        let result = ::destroy_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..], UMLToken::Destroy { name: "test".to_string() }));
    }

    #[test]
    fn test_par_parser() {
        let test_uml = r#"par
                            PERSON_A->PERSON_B:Test
                          else
                            note position
                              quick test
                            end note
                          end par
"#;

        let result = ::par_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Parallel {
                            sequences: vec![UMLTokens {
                                                tokens: vec![UMLToken::Message {
                                                                 from: "PERSON_A".to_string(),
                                                                 to: "PERSON_B".to_string(),
                                                                 text: Some("Test".to_string()),
                                                                 colour: None,
                                                             }],
                                            },
                                            UMLTokens {
                                                tokens: vec![UMLToken::Note {
                                                                 position: "position".to_string(),
                                                                 text: "quick test".to_string(),
                                                             }],
                                            }],
                        }))
    }

    #[test]
    fn test_nested_par() {
        let test_uml = r#"par
                            note position
                              outer par
                            end note
                            par
                              note position
                                inner par
                              end note
                            else
                              note position
                                inner
                              end note
                            end par
                          else
                            note position
                              outer else
                            end note
                          end par
"#;

        let result = ::par_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Parallel {
                            sequences: vec![UMLTokens {
                                                tokens: vec![UMLToken::Note {
                                                                 position: "position".to_string(),
                                                                 text: "outer par".to_string(),
                                                             },
                                                             UMLToken::Parallel {
                                                                 sequences: vec![UMLTokens {
                                                                                     tokens: vec![
                                            UMLToken::Note {
                                                position: "position".to_string(),
                                                text: "inner par".to_string()
                                            },
                                        ],
                                                                                 },
                                                                                 UMLTokens {
                                                                                     tokens: vec![
                                            UMLToken::Note {
                                                position: "position".to_string(),
                                                text: "inner".to_string()
                                            },
                                        ],
                                                                                 }],
                                                             }],
                                            },
                                            UMLTokens {
                                                tokens: vec![UMLToken::Note {
                                                                 position: "position".to_string(),
                                                                 text: "outer else".to_string(),
                                                             }],
                                            }],
                        }))
    }

    #[test]
    fn test_parse_tokens() {
        let test_uml = r#"PERSON_A->PERSON_B:Test
                          note position
                            quick test
                          end note
"#;

        let result = ::uml_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLTokens {
                            tokens: vec![UMLToken::Message {
                                             from: "PERSON_A".to_string(),
                                             to: "PERSON_B".to_string(),
                                             text: Some("Test".to_string()),
                                             colour: None,
                                         },
                                         UMLToken::Note {
                                             position: "position".to_string(),
                                             text: "quick test".to_string(),
                                         }],
                        }));
    }

    #[test]
    fn test_loop_parser() {
        let test_uml = r#"loop 10
                            note position
                              quick test
                            end note
                          end loop
"#;

        let result = ::loop_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Loop {
                            count: 10,
                            sequence: UMLTokens {
                                tokens: vec![UMLToken::Note {
                                                 position: "position".to_string(),
                                                 text: "quick test".to_string(),
                                             }],
                            },
                        }));
    }

    #[test]
    fn test_box_parser() {
        let test_uml = r#"box test
                            note position
                              quick test
                            end note
                          end box
"#;
        let result = ::box_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Box {
                            name: "test".to_string(),
                            sequence: UMLTokens {
                                tokens: vec![UMLToken::Note {
                                                 position: "position".to_string(),
                                                 text: "quick test".to_string(),
                                             }],
                            },
                        }));
    }

    #[test]
    fn test_uml_parser() {
        let test_uml = r#"@startuml
                          participant test1
                          @enduml
"#;
        let result = ::uml_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLTokens {
                            tokens: vec![UMLToken::StartUML,
                                         UMLToken::Participant {
                                             short_name: "test1".to_string(),
                                             long_name: None,
                                         },
                                         UMLToken::EndUML],
                        }));
    }

    #[test]
    fn test_blank_line_before_enduml() {
        let test_uml = r#"@startuml
                          participant test1

                          @enduml
"#;
        let result = ::uml_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLTokens {
                            tokens: vec![UMLToken::StartUML,
                                         UMLToken::Participant {
                                             short_name: "test1".to_string(),
                                             long_name: None,
                                         },
                                         UMLToken::EndUML],
                        }));
    }

    #[test]
    fn test_uml_parser_all() {
        let test_uml = r#"
@startuml
participant test1
note position
    quick test
end note
actor test

loop 5
    par test
        note position
            inside par
        end note
    else
        note position
            else clause
        end note
    end
end
activate test activate
deactivate test deactivate
@enduml
"#;
        let result = ::uml_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLTokens {
                            tokens: vec![UMLToken::StartUML,
                                         UMLToken::Participant {
                                             short_name: "test1".to_string(),
                                             long_name: None,
                                         },
                                         UMLToken::Note {
                                             position: "position".to_string(),
                                             text: "quick test".to_string(),
                                         },
                                         UMLToken::Participant {
                                             short_name: "test".to_string(),
                                             long_name: None,
                                         },
                                         UMLToken::Loop {
                                             count: 5,
                                             sequence: UMLTokens {
                                                 tokens: vec![UMLToken::Parallel {
                                                                  sequences: vec![UMLTokens {
                                                                                      tokens: vec![
                                            UMLToken::Note {
                                                position: "position".to_string(),
                                                text: "inside par".to_string()
                                            }
                                        ],
                                                                                  },
                                                                                  UMLTokens {
                                                                                      tokens: vec![
                                            UMLToken::Note {
                                                position: "position".to_string(),
                                                text: "else clause".to_string()
                                            }
                                        ],
                                                                                  }],
                                                              }],
                                             },
                                         },
                                         UMLToken::Activate { name: "test activate".to_string() },
                                         UMLToken::Deactivate {
                                             name: "test deactivate".to_string(),
                                         },
                                         UMLToken::EndUML],
                        }));
    }

    #[test]
    fn test_delay_token() {
        let test_uml = r#"
@startuml

delay 50

@enduml
"#;
        let result = ::uml_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLTokens {
                            tokens: vec![UMLToken::StartUML,
                                         UMLToken::Delay { text: "50".to_string() },
                                         UMLToken::EndUML],
                        }));
    }

    #[test]
    fn test_blank_line_before_par() {
        let test_uml = r#"
@startuml

par
  PERSON_A->PERSON_B: Hello 1
else
  PERSON_A->PERSON_B: Hello 2
else
  PERSON_A->PERSON_B: Hello 3
end par
@enduml
"#;
        let result = ::uml_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLTokens {
                            tokens: vec![UMLToken::StartUML,
                                         UMLToken::Parallel {
                                             sequences: vec![UMLTokens {
                                                                 tokens: vec![
                                UMLToken::Message {
                                    from: "PERSON_A".to_string(),
                                    to: "PERSON_B".to_string(),
                                    text: Some("Hello 1".to_string()),
                                    colour: None
                                }
                            ],
                                                             },
                                                             UMLTokens {
                                                                 tokens: vec![
                                UMLToken::Message {
                                    from: "PERSON_A".to_string(),
                                    to: "PERSON_B".to_string(),
                                    text: Some("Hello 2".to_string()),
                                    colour: None
                                }
                            ],
                                                             },
                                                             UMLTokens {
                                                                 tokens: vec![
                                UMLToken::Message {
                                    from: "PERSON_A".to_string(),
                                    to: "PERSON_B".to_string(),
                                    text: Some("Hello 3".to_string()),
                                    colour: None
                                }
                            ],
                                                             }],
                                         },
                                         UMLToken::EndUML],
                        }));
    }

    #[test]
    fn test_print_uml() {
        let test_uml = r#"@startuml
participant test1
note left
quick test
end note
participant test
loop 5
par
note left
inside par
end note
else
note left
else clause
end note
end par
end loop
activate test
deactivate test
alt
a->b:Hello
b->a
else
note left
second alt
end note
end alt
box test
participant contents
end box
@enduml
"#;
        let (_, uml_vector) = ::uml_parser(test_uml.as_bytes()).unwrap();

        let output_string = format!("{}", uml_vector);

        assert_eq!(output_string, test_uml);
    }

    #[test]
    fn test_alt_parser() {
        let test_uml = r#"alt
                            PERSON_A->PERSON_B:Test
                          else
                            note position
                              quick test
                            end note
                          end alt
"#;

        let result = ::alt_parser(test_uml.as_bytes());

        assert_eq!(result,
                   Done(&[][..],
                        UMLToken::Alt {
                            sequences: vec![UMLTokens {
                                                tokens: vec![UMLToken::Message {
                                                                 from: "PERSON_A".to_string(),
                                                                 to: "PERSON_B".to_string(),
                                                                 text: Some("Test".to_string()),
                                                                 colour: None,
                                                             }],
                                            },
                                            UMLTokens {
                                                tokens: vec![UMLToken::Note {
                                                                 position: "position".to_string(),
                                                                 text: "quick test".to_string(),
                                                             }],
                                            }],
                        }))
    }

    #[ignore]
    #[test]
    fn test_file_parser() {
        let file = std::path::PathBuf::from(file!())
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("test")
            .join("test.uml");
        let uml = parse_uml_file(file.to_str().unwrap(), None);

        assert_eq!(uml,
                   UMLTokens {
                       tokens: vec![UMLToken::StartUML,
                                    UMLToken::Participant {
                                        short_name: "test1".to_string(),
                                        long_name: None,
                                    },
                                    UMLToken::Note {
                                        position: "position".to_string(),
                                        text: "quick test".to_string(),
                                    },
                                    UMLToken::Participant {
                                        short_name: "test".to_string(),
                                        long_name: None,
                                    },
                                    UMLToken::Loop {
                                        count: 5,
                                        sequence: UMLTokens {
                                            tokens: vec![UMLToken::Parallel {
                                                             sequences: vec![UMLTokens {
                                                                                 tokens: vec![
                                    UMLToken::Note {
                                        position: "position".to_string(),
                                        text: "inside par".to_string()
                                    }
                                ],
                                                                             },
                                                                             UMLTokens {
                                                                                 tokens: vec![
                                    UMLToken::Note {
                                        position: "position".to_string(),
                                        text: "else clause".to_string()
                                    }
                                ],
                                                                             }],
                                                         }],
                                        },
                                    },
                                    UMLToken::Activate { name: "test activate".to_string() },
                                    UMLToken::Deactivate { name: "test deactivate".to_string() },
                                    UMLToken::EndUML],
                   });
    }
}
