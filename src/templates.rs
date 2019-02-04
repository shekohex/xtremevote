use rocket::{
  http::{ContentType, Status},
  response::{Responder, Result},
  Request, Response,
};
use std::io::Cursor;
use typed_html::{
  dom::DOMTree, elements::FlowContent, html, text,OutputType,
};

const STYLES: &str = include_str!("./style.css");

pub struct Html(pub DOMTree<String>);

impl<'r> Responder<'r> for Html {
  fn respond_to(self, _request: &Request) -> Result<'r> {
    Ok(
      Response::build()
        .status(Status::Ok)
        .header(ContentType::HTML)
        .sized_body(Cursor::new(self.0.to_string()))
        .finalize(),
    )
  }
}

// Function that wraps a DOM node in an HTML document
pub fn doc<T: OutputType + 'static>(
  page_title: &str,
  tree: Box<dyn FlowContent<T>>,
) -> DOMTree<T> {
  html!(
      <html>
        <head>
            <title>{ text!("{}", page_title) }</title>
            <meta charset="utf-8" />
            <meta name="viewport" content="width=device-width, initial-scale=1" />
            <script
              defer={true}
              src="https://use.fontawesome.com/releases/v5.7.1/js/all.js"
              integrity="sha384-eVEQC9zshBn0rFj4+TU78eNA19HMNigMviK/PU/FFjLXqa/GKPgX58rvt5Z8PLs7"
              crossorigin="anonymous"
            ></script>
            <link
              href="https://fonts.googleapis.com/css?family=Open+Sans:300,400,700"
              rel="stylesheet"
            />
            <link
              rel="stylesheet"
              href="https://cdnjs.cloudflare.com/ajax/libs/bulma/0.7.2/css/bulma.min.css"
            />
            <style>
               { text!("{}", STYLES) }
            </style>
          </head>
          <body>
            { tree }
             <script>
             r#"
              const btn = document.getElementById(`vote`);
              const usernameInput = document.getElementById(`username`);
              usernameInput.pattern = `[A-Za-z0-9]+`;
              usernameInput.placeholder = `Your Username`;
              btn.onclick = function(e) {
                const username = usernameInput.value;
                if (username !== ``) {
                  window.location.replace(`/vote/${username}`);
                } else {
                  alert(`Enter The Username`);
                }
              };
              "#
             </script>
          </body>
      </html>
  )
}
