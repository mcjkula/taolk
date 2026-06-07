use taolk::secret::{Phrase, Seed};
use taolk::types::MessageBody;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let phrase = Phrase::generate()?;
    let seed = Seed::from_phrase(&phrase);
    let signing_key = seed.derive_signing_key();
    let recipient = signing_key.public_key();

    let body = MessageBody::parse("hello from the SDK")?;
    let remark = samp::encode_public(&recipient, body.as_str());
    assert!(samp::is_samp_remark(remark.as_bytes()));

    Ok(())
}
