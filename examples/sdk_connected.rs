use taolk::event::Event;
use taolk::secret::Password;
use taolk::session::Session;
use taolk::types::{MessageBody, Pubkey};
use taolk::wallet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let password = Password::new("hunter2".into());
    let seed = wallet::open("demo", &password)?;

    let (session, rx) = Session::start(
        seed.as_bytes(),
        "wss://entrypoint-finney.opentensor.ai:443",
        "demo",
        true,
    )
    .await?;

    println!("Running as {}", session.ss58());

    let recipient = Pubkey::from_bytes([0xab; 32]);
    let body = MessageBody::parse("hello")?;
    let remark = session.build_encrypted_message(seed.as_bytes(), &recipient, &body)?;
    session.submit(&remark).await?;

    while let Ok(event) = rx.recv() {
        match event {
            Event::MessageSent => println!("Message confirmed on-chain"),
            Event::NewMessage {
                decrypted_body: Some(body),
                sender,
                ..
            } => println!("From {sender:?}: {body}"),
            Event::Error(e) => eprintln!("Error: {e}"),
            _ => {}
        }
    }

    Ok(())
}
