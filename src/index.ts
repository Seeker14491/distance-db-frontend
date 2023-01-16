import { Editor } from 'codemirror';
import * as either from 'fp-ts/lib/Either';
import * as t from 'io-ts';
import { Validation } from 'io-ts';
import { PathReporter } from 'io-ts/lib/PathReporter';
import 'regenerator-runtime/runtime';
import Split from 'split.js';
import DBGrid from './DBGrid';
import setUpEditor from './editor';

const SuccessResponse = t.type({
    success: t.type({
        lastUpdated: t.string,
        columnNames: t.array(t.string),
        rows: t.array(t.array(t.string)),
    }),
});
type SuccessResponse = t.TypeOf<typeof SuccessResponse>;

const ErrorResponse = t.type({ error: t.string });
type ErrorResponse = t.TypeOf<typeof ErrorResponse>;

const ApiResponse = t.union([SuccessResponse, ErrorResponse]);
type ApiResponse = t.TypeOf<typeof ApiResponse>;

const editorAndGrid = document.getElementById('editor-and-grid')!;
const editorDiv = document.getElementById('editor')!;
const eGridDiv = document.getElementById('grid')!;
const logDiv = document.getElementById('log')!;
const runQueryButton = document.getElementById(
    'run-query-button'
)! as HTMLButtonElement;
let grid: DBGrid | null = null;

(async () => {
    const query = readQueryFromAddressBar();
    let editor: Editor;
    if (query) {
        editor = setUpEditor(editorDiv, query);
    } else {
        editor = setUpEditor(editorDiv);
    }

    runQueryButton.onclick = async () => {
        runQueryButton.classList.add('loading');
        const sql = editor.getValue();
        logDiv.textContent = '';

        // set url query string
        window.history.replaceState(null, '', `?query=${encodeURIComponent(sql)}`);

        try {
            await queryDB(sql);
        } catch (e) {
            console.error(e);
            logDiv.innerText = e;
        } finally {
            runQueryButton.classList.remove('loading');
        }
    };

    Split([editorDiv, eGridDiv], {
        sizes: [50, 50],
        onDrag: () => {
            if (grid) {
                grid.api().sizeColumnsToFit();
            }
        }
    });
})();

async function queryDB(query: string): Promise<void> {
    const rawResponse = await fetch(`${process.env.SERVER_URL}?query=${encodeURIComponent(query)}`);
    const data = await rawResponse.json();

    const response = unwrapValidation(ApiResponse.decode(data));
    if (!isSuccessResponse(response)) {
        logDiv.textContent = response.error;
        return;
    }

    presentGrid(response.success.columnNames, response.success.rows);
}

function presentGrid(columnNames: string[], rows: string[][]): void {
    while (eGridDiv.firstChild) {
        eGridDiv.removeChild(eGridDiv.firstChild);
    }

    grid = new DBGrid(eGridDiv, columnNames, rows);
    const api = grid.api();

    api.sizeColumnsToFit();
    window.onresize = () => api.sizeColumnsToFit();
}

function unwrapValidation<T>(result: Validation<T>): T {
    if (either.isLeft(result)) {
        throw PathReporter.report(result).join('\n');
    }

    return result.right;
}

function isSuccessResponse(response: ApiResponse): response is SuccessResponse {
    return (response as ErrorResponse).error === undefined;
}

function readQueryFromAddressBar(): string | null {
    const urlParams = new URLSearchParams(window.location.search).entries();

    for (const pair of urlParams) {
        if (pair[0] === 'query') {
            return decodeURI(pair[1]);
        }
    }

    return null;
}
