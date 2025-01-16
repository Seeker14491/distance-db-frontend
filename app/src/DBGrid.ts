import {
    ColDef,
    ColumnApi,
    Grid,
    GridApi,
    GridOptions, ValueGetterParams,
} from 'ag-grid-community';

type SingleRowData = string[];
type DBGridOptions = Omit<GridOptions, 'rowData'> & { rowData: SingleRowData[] };
type DBValueGetterParams = Omit<ValueGetterParams, 'data'> & { data: SingleRowData };

export default class DBGrid {
    public readonly gridOptions: DBGridOptions;

    constructor(
        eGridDiv: HTMLElement,
        columnNames: string[],
        rows: string[][]
    ) {
        const columnDefs: ColDef[] = columnNames.map((name, i) => {
            return {
                headerName: name,
                valueGetter: (params: DBValueGetterParams) => params.data[i],
            };
        });

        this.gridOptions = {
            columnDefs,
            rowData: rows,
            onGridReady: x => x.api.sizeColumnsToFit(),
        };

        new Grid(eGridDiv, this.gridOptions);
    }

    public api(): GridApi {
        return this.gridOptions.api!;
    }

    public columnApi(): ColumnApi {
        return this.gridOptions.columnApi!;
    }
}
