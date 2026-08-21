use super::*;

impl NativeTextLabFragment {
    pub fn drive_presentation(&mut self, expected_bytes: usize) -> Result<(), String> {
        while self.presented.len() < expected_bytes {
            if !self.complete_host_request()? {
                match self
                    .scheduler
                    .step()
                    .map_err(|error| format!("{error:?}"))?
                {
                    SchedulerStatus::Progress { .. } => {}
                    SchedulerStatus::Idle => return Err("native presentation became idle".into()),
                    SchedulerStatus::Complete => {
                        return Err("native presentation completed too early".into())
                    }
                    SchedulerStatus::Cancelled => return Err("native Text Lab cancelled".into()),
                }
            }
        }
        Ok(())
    }

    pub fn close_return_input(&mut self) -> Result<(), String> {
        let (endpoint, cord) = self.endpoint(RemoteCordDirection::Ingress);
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn finish(&mut self) -> Result<(), String> {
        loop {
            if self.complete_host_request()? {
                continue;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => return Err("native Text Lab became idle at finish".into()),
                SchedulerStatus::Complete => return Ok(()),
                SchedulerStatus::Cancelled => return Err("native Text Lab cancelled".into()),
            }
        }
    }
}
