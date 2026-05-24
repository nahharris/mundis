import './styles.css';
import App from './App.svelte';
import { mount } from 'svelte';
import { logFrontendError, logFrontendInfo } from './logger';

window.addEventListener('error', (event) => {
  void logFrontendError('window.error', {
    message: event.message,
    filename: event.filename,
    lineno: event.lineno,
    colno: event.colno,
    error: event.error instanceof Error ? event.error.stack : String(event.error)
  });
});

window.addEventListener('unhandledrejection', (event) => {
  void logFrontendError('window.unhandledrejection', {
    reason: event.reason instanceof Error ? event.reason.stack : String(event.reason)
  });
});

void logFrontendInfo('frontend.boot');

const app = mount(App, {
  target: document.getElementById('app') as HTMLElement
});

export default app;
