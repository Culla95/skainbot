use std::collections::VecDeque;
use serenity::model::id::UserId;
use parking_lot::Mutex;
use std::time::Duration;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Track {
    pub title: String,
    pub channel: String,
    pub url: String,
    pub duration: Option<Duration>,
    pub requester: UserId,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug)]
pub struct QueueManager {
    queue: Mutex<VecDeque<Track>>,
}

#[allow(dead_code)]
impl QueueManager {
    pub fn new() -> self::QueueManager {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }

    pub fn add(&self, track: Track) {
        let mut queue = self.queue.lock();
        queue.push_back(track);
    }

    pub fn add_front(&self, track: Track) {
        let mut queue = self.queue.lock();
        queue.push_front(track);
    }

    pub fn pop(&self) -> Option<Track> {
        let mut queue = self.queue.lock();
        queue.pop_front()
    }

    // Returns a list of tracks for the !queue command (limit to N)
    pub fn list(&self, limit: usize) -> Vec<Track> {
        let queue = self.queue.lock();
        queue.iter().take(limit).cloned().collect()
    }

    pub fn clear(&self) {
        let mut queue = self.queue.lock();
        queue.clear();
    }
    
    pub fn len(&self) -> usize {
        let queue = self.queue.lock();
        queue.len()
    }

    pub fn is_empty(&self) -> bool {
        let queue = self.queue.lock();
        queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_add_pop() {
        let queue = QueueManager::new();
        let track = Track {
            title: "Test".to_string(),
            channel: "Channel".to_string(),
            url: "http://url".to_string(),
            duration: None,
            requester: UserId::new(1),
            thumbnail_url: None,
        };
        
        queue.add(track.clone());
        assert_eq!(queue.len(), 1);
        
        let popped = queue.pop();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().title, "Test");
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_clear() {
        let queue = QueueManager::new();
        let track = Track {
            title: "Test".to_string(),
            channel: "Channel".to_string(),
            url: "http://url".to_string(),
            duration: None,
            requester: UserId::new(1),
            thumbnail_url: None,
        };
        queue.add(track.clone());
        queue.add(track.clone());
        
        assert_eq!(queue.len(), 2);
        queue.clear();
        assert_eq!(queue.len(), 0);
    }
}

