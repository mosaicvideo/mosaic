import { getCurrentWebview } from '@tauri-apps/api/webview';

export function wireDropzone(el, onPaths) {
  const webview = getCurrentWebview();
  webview.onDragDropEvent((event) => {
    if (event.payload.type === 'over') el.classList.add('drag');
    else if (event.payload.type === 'drop') {
      el.classList.remove('drag');
      onPaths(event.payload.paths || []);
    } else el.classList.remove('drag');
  });
}
