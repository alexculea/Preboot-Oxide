use anyhow::Error;
use async_std::task;
use async_tftp::server::TftpServerBuilder;
use log::info;

use crate::conf::Conf;
use crate::Result;


pub fn spawn_tftp_service_async(conf: &Conf) -> Result<()> {

  if let Some(tftp_path) = conf.get_tftp_serve_path() {
    let tftp_dir = tftp_path.clone();
    task::spawn(async {
      let tftpd = TftpServerBuilder::with_dir_ro(tftp_path)?
      .build()
      .await?;


      tftpd.serve().await?;

      async_tftp::Result::<(), Error>::Ok(())
    });

    info!("TFTP server started on path: {}", tftp_dir);
} else {
    info!("TFTP server not started, no path configured.");
}

  Ok(())   
}
