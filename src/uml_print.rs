use {UMLToken, UMLTokens};
use std::fmt;
use std::ops::Deref;

impl fmt::Display for UMLTokens {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut tokens_str = String::new();

        for token in &self.tokens {
            tokens_str.push_str(&format!("{}", token));
        }

        write!(f, "{}", tokens_str)
    }
}

impl fmt::Display for UMLToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        let uml_str = match *self {
            UMLToken::StartUML => "@startuml\n".to_string(),

            UMLToken::EndUML => "@enduml\n".to_string(),

            UMLToken::Note {
                ref position,
                ref text,
            } => format!("note {}\n{}\nend note\n", position, text),

            UMLToken::Parallel { ref sequences } => {
                let mut par_str = "par\n".to_string();
                let mut first_loop = true;

                for sequence in sequences.deref() {
                    if !first_loop {
                        par_str.push_str("else\n");
                    }

                    par_str.push_str(&format!("{}", sequence));

                    first_loop = false;
                }

                par_str.push_str("end par\n");

                par_str
            }

            UMLToken::Alt { ref sequences } => {
                let mut par_str = "alt\n".to_string();
                let mut first_loop = true;

                for sequence in sequences.deref() {
                    if !first_loop {
                        par_str.push_str("else\n");
                    }

                    par_str.push_str(&format!("{}", sequence));

                    first_loop = false;
                }

                par_str.push_str("end alt\n");

                par_str
            }

            UMLToken::Message {
                ref from,
                ref to,
                ref text,
                ref colour,
            } => {
                let seperator = match *colour {
                    Some(ref colour) => format!("-[#{}]>", colour),
                    None => "->".to_string(),
                };

                let mut msg_str = format!("{}{}{}", from, seperator, to);

                if let Some(ref text) = *text {
                    msg_str.push_str(&format!(":{}", text))
                }

                msg_str.push_str("\n");

                msg_str
            }

            UMLToken::Participant {
                ref long_name,
                ref short_name,
            } => {
                let (name1, name2) = match *long_name {
                    Some(ref name) => (name.to_string(), Some(short_name.to_string())),
                    None => (short_name.to_string(), None),
                };

                let mut participant_str = format!("participant {}", name1);

                if name2.is_some() {
                    participant_str.push_str(&format!(" as {}", name2.unwrap()));
                }

                participant_str.push_str("\n");

                participant_str
            }

            UMLToken::Activate { ref name } => format!("activate {}\n", name),

            UMLToken::Deactivate { ref name } => format!("deactivate {}\n", name),

            UMLToken::Loop {
                ref sequence,
                ref count,
            } => {
                let mut loop_str = format!("loop {}\n", count);

                loop_str.push_str(&format!("{}", sequence));

                loop_str.push_str("end loop\n");

                loop_str
            }

            UMLToken::Box {
                ref name,
                ref sequence,
            } => {
                let mut box_str = format!("box {}\n", name);

                box_str.push_str(&format!("{}", sequence));

                box_str.push_str("end box\n");

                box_str
            }

            UMLToken::Include { ref sequence, .. } => format!("{}", sequence),

            UMLToken::Destroy { ref name } => format!("destroy {}\n", name),

            UMLToken::Delay { ref text } => format!("delay {}\n", text),
        };

        write!(f, "{}", uml_str)
    }
}
