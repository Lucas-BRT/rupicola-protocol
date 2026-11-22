import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import './style.css';
function Counter() {
  const [count, setCount] = useState(0);
  return <main><h1>Counter</h1><output aria-live="polite">{count}</output><div><button onClick={() => setCount(count + 1)}>Increment</button> <button onClick={() => setCount(0)}>Reset</button></div></main>;
}
createRoot(document.getElementById('root')).render(<Counter />);
