import { useState } from "react";
import { Link } from "@tanstack/react-router";
import { type User, type BookRef } from "@/lib/types";
import { date, money } from "@/lib/format";
import { useData, DataState, DataTable, Pagination } from "@/components/data";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Field, OperationDialog, useOperations } from "@/components/operation";
import { AllowancePanel, HistoryPanel } from "./allowance";
import { ChargesPanel } from "./charges";
import { ActivitySummary, UsagePage } from "./usage";

export function UsersPage() {
  const [offset, setOffset] = useState(0),
    [search, setSearch] = useState(""),
    [draft, setDraft] = useState(""),
    [disabled, setDisabled] = useState("");
  const [create, setCreate] = useState(false);
  const ops = useOperations();
  const query = useData<{ users: User[]; total: number }>(
    `/admin/users?limit=20&offset=${offset}&search=${encodeURIComponent(search)}${disabled ? `&disabled=${disabled}` : ""}`,
  );
  return (
    <>
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1>账号管理</h1>
          <p className="mt-2 text-sm text-muted-foreground">
            开通阅读账号，管理材料与 AI 使用额度。
          </p>
        </div>
        <Button disabled={ops.blocked} onClick={() => setCreate(true)}>
          创建账号
        </Button>
      </div>
      <form
        className="flex flex-wrap items-end gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          setOffset(0);
          setSearch(draft);
        }}
      >
        <Field label="搜索账号或邮箱">
          <Input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="输入账号或邮箱"
          />
        </Field>
        <Field label="账号状态">
          <select
            value={disabled}
            onChange={(e) => {
              setDisabled(e.target.value);
              setOffset(0);
            }}
          >
            <option value="">全部状态</option>
            <option value="false">启用</option>
            <option value="true">停用</option>
          </select>
        </Field>
        <Button variant="outline" type="submit" className="mb-2">
          搜索
        </Button>
      </form>
      <DataState query={query} />
      {query.data && (
        <>
          <div className="rounded-lg border">
            <DataTable
              rows={query.data.users}
              columns={[
                {
                  header: "账号",
                  accessorKey: "user_id",
                  cell: ({ row }) => (
                    <Link
                      className="font-medium underline underline-offset-4"
                      to="/users/$userId"
                      params={{ userId: row.original.user_id }}
                    >
                      {row.original.user_id}
                    </Link>
                  ),
                },
                {
                  header: "邮箱",
                  cell: ({ row }) => row.original.email ?? "未绑定",
                },
                {
                  header: "状态",
                  cell: ({ row }) => (
                    <Badge
                      variant={row.original.disabled ? "secondary" : "outline"}
                    >
                      {row.original.disabled ? "停用" : "启用"}
                      {row.original.is_admin ? " · 管理员" : ""}
                    </Badge>
                  ),
                },
                {
                  header: "可用额度（元）",
                  cell: ({ row }) =>
                    row.original.current_allowance
                      ? money(
                          row.original.current_allowance.balance
                            .available_micro_cny,
                        )
                      : "无有效额度期",
                },
                {
                  header: "到期时间（香港）",
                  cell: ({ row }) =>
                    date(row.original.current_allowance?.expires_at),
                },
                {
                  header: "最近阅读 / 提问",
                  cell: ({row}) => <span>阅读：{row.original.activity.last_read_at == null ? '暂无记录' : date(row.original.activity.last_read_at)}<br />提问：{row.original.activity.last_question_at == null ? '暂无记录' : date(row.original.activity.last_question_at)}</span>,
                },
              ]}
            />
          </div>
          <Pagination
            offset={offset}
            total={query.data.total}
            change={setOffset}
          />
        </>
      )}
      <OperationDialog
        title="创建账号"
        description="创建普通读者账号。创建后可在详情中授权材料并授予额度。"
        open={create}
        onOpenChange={setCreate}
        onSubmit={(data) =>
          ops.submit("创建账号", "/admin/users", {
            user_id: String(data.get("user_id")).trim(),
            password: String(data.get("password")),
          })
        }
      >
        <Field label="新账号">
          <Input name="user_id" required autoComplete="off" />
        </Field>
        <Field label="初始密码">
          <Input
            name="password"
            type="password"
            required
            autoComplete="new-password"
          />
        </Field>
      </OperationDialog>
    </>
  );
}

export function UserPage({ userId }: { userId: string }) {
  const [offset, setOffset] = useState(0),
    [action, setAction] = useState("");
  const query = useData<User>(
    `/admin/users/${encodeURIComponent(userId)}?limit=20&offset=${offset}`,
  );
  const ops = useOperations(),
    base = `/admin/users/${encodeURIComponent(userId)}`;
  const user = query.data;
  return (
    <>
      <Link to="/users" className="text-sm underline">
        ← 全部账号
      </Link>
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div><h1>{userId}</h1><p className="mt-2 break-all text-sm">{user?.email ?? "未绑定邮箱"}</p></div>
        {user && (
          <div className="flex flex-wrap gap-2">
            <Badge variant="outline">
              {user.disabled ? "已停用" : "启用中"}
            </Badge>
            {["改密", "撤销会话", user.disabled ? "启用" : "停用"].map(
              (label) => (
                <Button
                  key={label}
                  variant="outline"
                  disabled={ops.blocked}
                  onClick={() => setAction(label)}
                >
                  {label}
                </Button>
              ),
            )}
          </div>
        )}
      </div>
      <DataState query={query} />
      {user && (
        <>
          <ActivitySummary activity={user.activity} />
          <AllowancePanel userId={userId} current={user.current_allowance} />
          <Materials user={user} offset={offset} setOffset={setOffset} />
          <ChargesPanel userId={userId} />
          <HistoryPanel userId={userId} />
          <UsagePage userId={userId} />
          <OperationDialog
            title={`${action} · ${userId}`}
            description={
              action === "改密"
                ? "改密会撤销此账号已有登录会话。"
                : action === "停用"
                  ? "停用会撤销登录会话并取消排队和活动运行，保留已发生费用及阅读成果。"
                  : action === "启用"
                    ? "启用后需要重新登录，已有任务不会自动重跑。"
                    : "撤销此账号的所有登录会话；需要重新登录。"
            }
            open={!!action}
            onOpenChange={(open) => {
              if (!open) setAction("");
            }}
            onSubmit={(data) =>
              ops.submit(
                `${action} ${userId}`,
                `${base}/${action === "改密" ? "password" : action === "撤销会话" ? "revoke-sessions" : "status"}`,
                action === "改密"
                  ? { password: String(data.get("password")) }
                  : action === "撤销会话"
                    ? {}
                    : { disabled: action === "停用" },
              )
            }
          >
            {action === "改密" && (
              <Field label="新密码">
                <Input
                  name="password"
                  type="password"
                  required
                  autoComplete="new-password"
                />
              </Field>
            )}
          </OperationDialog>
        </>
      )}
    </>
  );
}

function Materials({
  user,
  offset,
  setOffset,
}: {
  user: User;
  offset: number;
  setOffset: (value: number) => void;
}) {
  const [open, setOpen] = useState(false),
    [bookOffset, setBookOffset] = useState(0),
    [revoke, setRevoke] = useState<BookRef | null>(null);
  const books = useData<{
    books: { published_book_ref: BookRef; is_default: boolean }[];
    total: number;
  }>(`/admin/books?limit=20&offset=${bookOffset}`);
  const ops = useOperations(),
    base = `/admin/users/${encodeURIComponent(user.user_id)}`;
  return (
    <section className="space-y-3 rounded-lg border p-5">
      <div className="flex items-center justify-between">
        <h2>材料授权</h2>
        <Button
          variant="outline"
          disabled={ops.blocked}
          onClick={() => setOpen(true)}
        >
          授权材料
        </Button>
      </div>
      <DataTable
        rows={user.book_grants.items}
        columns={[
          { header: "材料", accessorKey: "book_id" },
          { header: "发布版本", accessorKey: "publication_id" },
          {
            header: "操作",
            cell: ({ row }) => (
              <Button
                variant="ghost"
                disabled={ops.blocked}
                onClick={() => setRevoke(row.original)}
              >
                撤销授权
              </Button>
            ),
          },
        ]}
      />
      <Pagination
        offset={offset}
        total={user.book_grants.total}
        change={setOffset}
      />
      <OperationDialog
        title="授权材料"
        description="选择已登记的确切发布版本。"
        open={open}
        onOpenChange={setOpen}
        onSubmit={(data) =>
          ops.submit("授权材料", `${base}/book-grants`, {
            published_book_ref: JSON.parse(String(data.get("book"))),
          })
        }
      >
        <DataState query={books} />
        {books.data && (
          <>
            <Field label="发布材料">
              <select name="book" required defaultValue="">
                <option value="" disabled>
                  请选择材料
                </option>
                {books.data.books.map((book) => (
                  <option
                    key={JSON.stringify(book.published_book_ref)}
                    value={JSON.stringify(book.published_book_ref)}
                  >
                    {book.published_book_ref.book_id} ·{" "}
                    {book.published_book_ref.publication_id}
                    {book.is_default ? "（默认）" : ""}
                  </option>
                ))}
              </select>
            </Field>
            <Pagination
              offset={bookOffset}
              total={books.data.total}
              change={setBookOffset}
            />
          </>
        )}
      </OperationDialog>
      <OperationDialog
        title="撤销材料授权"
        description={`撤销 ${revoke?.book_id ?? ""} · ${revoke?.publication_id ?? ""} 的访问权限。`}
        open={!!revoke}
        onOpenChange={(open) => {
          if (!open) setRevoke(null);
        }}
        onSubmit={() =>
          ops.submit("撤销材料授权", `${base}/book-revocations`, {
            published_book_ref: revoke,
          })
        }
      />
    </section>
  );
}
