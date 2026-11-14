use std::io::Write;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

pub struct ByteCounterReadStream<R :Read> {
    inner : R,
    counter : Arc<AtomicUsize>,
}
impl<R: Read> ByteCounterReadStream<R> {
    pub fn new(reader : R) -> Self {
        Self{
            inner : reader,
            counter : Arc::new(AtomicUsize::new(0)),
        }
    }
    pub fn get_handle(&self) -> Arc<AtomicUsize> {
        self.counter.clone()
    }
}
impl<R: Read> Read for ByteCounterReadStream<R> {

    fn read(& mut self, buf: & mut [u8]) -> Result<usize, std::io::Error>{
        let n_read = self.inner.read(buf)?;
        if n_read != 0{
            self.counter.fetch_add(n_read, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(n_read)
    }
}

pub struct ByteCounterWriteStream<W :Write> {
    inner : W,
    counter : Arc<AtomicUsize>,
}
impl<W :Write> ByteCounterWriteStream<W> {
    pub fn new(writer :  W) -> ByteCounterWriteStream<W> {
        Self{
            inner: writer,
            counter : Arc::new(AtomicUsize::new(0)),
        }
    }
    pub fn get_handle(&self) -> Arc<AtomicUsize> {
        self.counter.clone()
    }
}
impl<W :Write> Write for ByteCounterWriteStream<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n_write = self.inner.write(buf)?;
        if n_write > 0{
            self.counter.fetch_add(n_write, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(n_write)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}