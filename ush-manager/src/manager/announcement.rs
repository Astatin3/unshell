use crate::Manager;
use unshell::Announcement;

impl Manager {
    pub fn recv_announcement(&mut self, announcement: &Announcement) {
        match announcement {
            Announcement::TestAnnouncement(str) => {
                println!("Got test announcement: {}", str)
            } // Announcement::GetRuntimes => todo!(),
              // Announcement::GetRuntimesAck(_) => todo!(),
              // Announcement::StartRuntime(runtime_config) => todo!(),
              // Announcement::StartRuntimeAck(_) => todo!(),
              // _ => {}
        }
    }
}
