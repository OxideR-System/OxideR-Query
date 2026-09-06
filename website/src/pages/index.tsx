import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import CodeBlock from '@theme/CodeBlock';
import Heading from '@theme/Heading';

import styles from './index.module.css';

const EXAMPLE = `use oxider_query::prelude::*;

#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i32,
    email: Option<String>,
}

let rendered = User::query()
    .select((User::id, User::name))
    .filter(User::name.contains("nguyen").and(User::age.ge(18)))
    .order_by(User::id.desc())
    .limit(20)
    .to_sql(&Postgres)?;

// SELECT "users"."id", "users"."name" FROM "users"
// WHERE "users"."name" LIKE $1 ESCAPE '!' AND "users"."age" >= $2
// ORDER BY "users"."id" DESC LIMIT 20
`;

type Feature = {
  title: string;
  body: ReactNode;
};

const FEATURES: Feature[] = [
  {
    title: 'Trình biên dịch bắt lỗi, không phải database',
    body: (
      <>
        Sai kiểu toán hạng, quên một <code>JOIN</code>, dùng aggregate lên cột
        không hợp lệ: tất cả đều là lỗi biên dịch. Kiểm tra phạm vi bảng ở tầng
        type là thứ QueryDSL trên JVM không làm được.
      </>
    ),
  },
  {
    title: 'Một query, ba engine',
    body: (
      <>
        Query dựng ra là AST độc lập dialect. <code>to_sql</code> sinh cặp{' '}
        <code>(sql, params)</code> cho PostgreSQL, MySQL hoặc SQLite, kể cả khi
        ba engine viết cùng một ý theo ba cách khác nhau.
      </>
    ),
  },
  {
    title: 'Giả lập khi giống hệt, từ chối khi không',
    body: (
      <>
        <code>NULLS LAST</code> và <code>FILTER</code> được giả lập cho kết quả y
        hệt bản native. Thứ không giả lập đúng được thì bị từ chối ngay lúc
        render, thay vì sinh SQL sai rồi để database báo lỗi khó hiểu.
      </>
    ),
  },
  {
    title: 'Không có bước sinh mã',
    body: (
      <>
        <code>#[derive(Entity)]</code> sinh metamodel ngay tại chỗ. Không có
        annotation processor, không có thư mục mã sinh ra, không có kiểu{' '}
        <code>QUser</code> thứ hai phải import.
      </>
    ),
  },
  {
    title: 'Escape đúng ngay từ đầu',
    body: (
      <>
        Tìm chuỗi <code>50%</code> không khớp <code>500 units</code>. Giá trị
        được escape vô điều kiện và mệnh đề <code>ESCAPE</code> luôn được phát
        ra, kể cả với mẫu động.
      </>
    ),
  },
  {
    title: 'Bề mặt SQL đầy đủ',
    body: (
      <>
        Mọi loại join, self-join qua alias, window function, CTE đệ quy, set
        operation, subquery tương quan, upsert, <code>RETURNING</code>, khóa
        dòng.
      </>
    ),
  },
];

function Hero() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <Heading as="h1" className="hero__title">
          {siteConfig.title}
        </Heading>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div className={styles.buttons}>
          <Link className="button button--secondary button--lg" to="/docs/">
            Đọc tài liệu
          </Link>
          <Link
            className="button button--outline button--secondary button--lg"
            to="/docs/getting-started">
            Bắt đầu nhanh
          </Link>
        </div>
      </div>
    </header>
  );
}

export default function Home(): ReactNode {
  return (
    <Layout
      title="Tài liệu"
      description="OxideR-Query: dựng câu lệnh SQL type-safe, đa dialect cho Rust, lấy cảm hứng từ QueryDSL.">
      <Hero />
      <main>
        <section className={styles.section}>
          <div className="container">
            <div className="row">
              <div className="col col--8 col--offset-2">
                <CodeBlock language="rust">{EXAMPLE}</CodeBlock>
              </div>
            </div>
          </div>
        </section>
        <section className={styles.section}>
          <div className="container">
            <div className="row">
              {FEATURES.map((feature) => (
                <div
                  key={feature.title}
                  className="col col--4 margin-bottom--lg">
                  <Heading as="h3">{feature.title}</Heading>
                  <p>{feature.body}</p>
                </div>
              ))}
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
