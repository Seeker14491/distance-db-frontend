import CodeMirror from 'codemirror';
import { Editor } from 'codemirror';
import 'codemirror/lib/codemirror.css';
import 'codemirror/mode/sql/sql';
import 'codemirror/theme/material-darker.css'

export default function setUpEditor(
    domElement: HTMLElement,
    sqlCode: string = 'SELECT *\nFROM users\nLIMIT 10'
): Editor {
    return CodeMirror(domElement, {
        value: sqlCode,
        theme: 'material-darker',
        lineNumbers: true,
        autofocus: true,
    });
}
