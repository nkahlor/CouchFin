//! Checks a real Jellyfin server by hand: public info, login, then
//! `/Users/Me`. Afterwards the server's Dashboard > Devices lists "Couchfin".
//!
//! ```sh
//! cargo run -p couchfin-api --example probe -- http://jellyfin.lan:8096 alice
//! ```
//!
//! The password is read from the terminal without echo, and is never printed.
//! Each run uses a new random device ID unless `COUCHFIN_DEVICE_ID` is set, so
//! set it to keep one Devices entry across runs.

use std::process::ExitCode;

use couchfin_api::{DeviceId, JellyfinApi, JellyfinClient, Password, ServerUrl, Session};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(url), Some(username), None) = (args.next(), args.next(), args.next()) else {
        return Err("usage: probe <server-url> <username>".into());
    };
    let server: ServerUrl = url.parse()?;
    let device_id = match std::env::var("COUCHFIN_DEVICE_ID") {
        Ok(id) => id.parse()?,
        Err(_) => DeviceId::generate(),
    };
    let client = JellyfinClient::new(device_id)?;
    println!("server:    {server}");
    println!("device ID: {}", client.device_id());

    let runtime = tokio::runtime::Runtime::new()?;
    let info = runtime.block_on(client.public_system_info(&server))?;
    println!(
        "found:     {} (Jellyfin {})",
        info.server_name, info.version
    );

    let password = Password::new(rpassword::prompt_password(format!(
        "password for {username}: "
    ))?);
    let login = runtime.block_on(client.authenticate_by_name(&server, &username, &password))?;
    println!("logged in: {}", login.user.name);

    let session = Session::new(server, login.access_token);
    let me = runtime.block_on(client.current_user(&session))?;
    println!("/Users/Me: {} (200 OK)", me.name);
    println!(
        "Dashboard > Devices should now list \"{}\"",
        couchfin_api::DEVICE_NAME
    );
    Ok(())
}
