pub trait BackgroundWidget: ratatui::widgets::StatefulWidget {
    type Event;
    fn spawn(self) -> tokio::task::JoinHandle<()>;
}
