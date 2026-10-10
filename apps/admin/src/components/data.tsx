import { useQuery } from "@tanstack/react-query";
import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { session } from "@/lib/api";
import { Button } from "./ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "./ui/table";

export function useData<T>(path: string) {
  return useQuery({
    queryKey: [session.snapshot().epoch, path],
    queryFn: ({ signal }) => session.request<T>(path, "GET", undefined, signal),
  });
}
export function DataState({
  query,
}: {
  query: { isPending: boolean; error: Error | null; refetch: () => unknown };
}) {
  if (query.isPending)
    return (
      <p role="status" className="py-6 text-muted-foreground">
        正在读取…
      </p>
    );
  if (query.error)
    return (
      <div role="alert" className="py-4 text-destructive">
        {query.error.message}{" "}
        <Button variant="outline" onClick={() => void query.refetch()}>
          重新读取
        </Button>
      </div>
    );
  return null;
}
export function DataTable<T>({
  rows,
  columns,
}: {
  rows: T[];
  columns: ColumnDef<T>[];
}) {
  const table = useReactTable({
    data: rows,
    columns,
    getCoreRowModel: getCoreRowModel(),
  });
  return (
    <Table>
      <TableHeader>
        {table.getHeaderGroups().map((group) => (
          <TableRow key={group.id}>
            {group.headers.map((header) => (
              <TableHead key={header.id}>
                {flexRender(
                  header.column.columnDef.header,
                  header.getContext(),
                )}
              </TableHead>
            ))}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {table.getRowModel().rows.map((row) => (
          <TableRow key={row.id}>
            {row.getVisibleCells().map((cell) => (
              <TableCell key={cell.id}>
                {flexRender(cell.column.columnDef.cell, cell.getContext())}
              </TableCell>
            ))}
          </TableRow>
        ))}
        {rows.length === 0 && (
          <TableRow>
            <TableCell
              colSpan={columns.length}
              className="py-8 text-center text-muted-foreground"
            >
              暂无记录
            </TableCell>
          </TableRow>
        )}
      </TableBody>
    </Table>
  );
}
export function Pagination({
  offset,
  total,
  change,
  size = 20,
}: {
  offset: number;
  total: number;
  change: (value: number) => void;
  size?: number;
}) {
  return (
    <div className="flex items-center justify-end gap-3 py-3 text-sm">
      <span>
        共 {total} 条 · 第 {Math.floor(offset / size) + 1} 页
      </span>
      <Button
        type="button"
        variant="outline"
        disabled={!offset}
        onClick={() => change(Math.max(0, offset - size))}
      >
        上一页
      </Button>
      <Button
        type="button"
        variant="outline"
        disabled={offset + size >= total}
        onClick={() => change(offset + size)}
      >
        下一页
      </Button>
    </div>
  );
}
