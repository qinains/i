import {node} from './render.js';
export function mountScratchControls({create,clear,download}){
    const label=node('label');const caption=node('span','Upload destination');caption.dataset.i18n='Upload destination';label.append(caption);const target=node('select');target.id='upload-target';
    for(const [value,text]of [['','/workspace'],['/tmp/','/tmp (cleared on reload)']]){const option=node('option',text);option.value=value;option.dataset.i18n=text;target.append(option);}
    label.append(target);document.getElementById('btn-upload').before(label);
    const note=node('p','/tmp is separate scratch space. Reload clears it; normal ZIP excludes it.','note');
    note.dataset.i18n=note.textContent;
    const clearButton=node('button','Clear /tmp…');clearButton.id='btn-clear-tmp';clearButton.addEventListener('click',clear);
    const downloadButton=node('button','Download /tmp ZIP');downloadButton.id='btn-download-tmp';downloadButton.addEventListener('click',download);
    clearButton.dataset.i18n=clearButton.textContent;downloadButton.dataset.i18n=downloadButton.textContent;
    document.querySelector('#file-actions .file-actions-body').append(note,clearButton,downloadButton);
}
