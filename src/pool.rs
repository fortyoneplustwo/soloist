use std::{
    io,
    sync::{
        Arc, Mutex,
        mpsc::{self},
    },
    thread::{self, JoinHandle},
};

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Worker {
    id: usize,
    handle: JoinHandle<()>,
}

impl Worker {
    fn new(id: usize, jobs_chan: Arc<Mutex<mpsc::Receiver<Job>>>) -> Result<Self, io::Error> {
        let handle = thread::Builder::new().spawn(move || {
            loop {
                let msg = jobs_chan
                    .lock()
                    .expect("Worker thread failed to acquire jobs channel lock.")
                    .recv();
                match msg {
                    Ok(job) => {
                        println!("Worker {id} got a job; executing.");
                        job();
                    }
                    Err(_) => {
                        println!("Worker {id} disconnected; shutting down.");
                        break;
                    }
                }
            }
        })?;

        Ok(Worker { id, handle })
    }
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    jobs_chan: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The `size` is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> Result<ThreadPool, io::Error> {
        assert!(size > 0);
        let mut workers: Vec<Worker> = Vec::with_capacity(size);

        let (sender, recver) = mpsc::channel();
        let shared_recver = Arc::new(Mutex::new(recver));

        for i in 0..size {
            let id = i + 1;
            let w = Worker::new(id, Arc::clone(&shared_recver))?;
            workers.push(w);
        }

        Ok(ThreadPool {
            workers,
            jobs_chan: Some(sender),
        })
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.jobs_chan
            .as_ref()
            .unwrap()
            .send(job)
            .unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for worker in self.workers.drain(..) {
            drop(self.jobs_chan.take());
            println!("Shutting down worker {}", worker.id);
            // WARN: this could panic! Might have to rework this!
            worker.handle.join().unwrap();
        }
    }
}
