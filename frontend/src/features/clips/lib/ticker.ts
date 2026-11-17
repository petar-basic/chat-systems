const SOURCE = 'let t;onmessage=(e)=>{clearInterval(t);if(e.data>0)t=setInterval(()=>postMessage(0),e.data)}';

export function tick(everyMs: number, onTick: () => void): () => void {
  if (typeof Worker === 'undefined') {
    const timer = window.setInterval(onTick, everyMs);
    return () => window.clearInterval(timer);
  }
  const url = URL.createObjectURL(new Blob([SOURCE], { type: 'text/javascript' }));
  const worker = new Worker(url);
  worker.onmessage = onTick;
  worker.postMessage(everyMs);
  return () => {
    worker.terminate();
    URL.revokeObjectURL(url);
  };
}
