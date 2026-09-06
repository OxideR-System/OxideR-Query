//! INSERT, UPDATE, and DELETE rendering.

use crate::ast::dml::{
    Assignment, ConflictAction, DeleteAst, InsertAst, InsertSource, OnConflict, UpdateAst,
};
use crate::ast::node::Node;
use crate::ast::query::Source;
use crate::dialect::Dialect;
use crate::render::{RenderResult, Rendered, Renderer};

/// Render an INSERT for a dialect.
pub fn render_insert(statement: &InsertAst, dialect: &dyn Dialect) -> RenderResult<Rendered> {
    let mut renderer = Renderer::new(dialect);
    renderer.insert(statement)?;
    Ok(renderer.finish())
}

/// Render an UPDATE for a dialect.
pub fn render_update(statement: &UpdateAst, dialect: &dyn Dialect) -> RenderResult<Rendered> {
    let mut renderer = Renderer::new(dialect);
    renderer.update(statement)?;
    Ok(renderer.finish())
}

/// Render a DELETE for a dialect.
pub fn render_delete(statement: &DeleteAst, dialect: &dyn Dialect) -> RenderResult<Rendered> {
    let mut renderer = Renderer::new(dialect);
    renderer.delete(statement)?;
    Ok(renderer.finish())
}

impl Renderer<'_> {
    /// Render an INSERT statement.
    fn insert(&mut self, statement: &InsertAst) -> RenderResult {
        self.push("INSERT INTO ");
        self.table_name(&statement.table);

        if !statement.columns.is_empty() {
            self.push(" (");
            self.comma_separated(&statement.columns, |r, name| {
                r.ident(name);
                Ok(())
            })?;
            self.push(")");
        }

        match &statement.source {
            InsertSource::Values(rows) => {
                if rows.is_empty() {
                    return Err(crate::render::RenderError::Invalid(
                        "an INSERT needs at least one row of values",
                    ));
                }
                if rows.len() > 1 && !self.dialect().caps().multi_row_insert {
                    return self.unsupported("multi-row INSERT");
                }
                self.push(" VALUES ");
                self.comma_separated(rows, |r, row| {
                    r.push("(");
                    r.comma_separated(row, |r, value| r.expr(value))?;
                    r.push(")");
                    Ok(())
                })?;
            }
            InsertSource::Query(query) => {
                self.push(" ");
                self.select(query)?;
            }
        }

        if let Some(conflict) = &statement.on_conflict {
            self.on_conflict(conflict)?;
        }
        self.returning(&statement.returning)?;
        Ok(())
    }

    /// Render a conflict-handling clause in whichever form the dialect speaks.
    fn on_conflict(&mut self, conflict: &OnConflict) -> RenderResult {
        let caps = self.dialect().caps();
        if caps.on_conflict {
            self.push(" ON CONFLICT");
            if !conflict.target.is_empty() {
                self.push(" (");
                self.comma_separated(&conflict.target, |r, name| {
                    r.ident(name);
                    Ok(())
                })?;
                self.push(")");
            }
            match &conflict.action {
                ConflictAction::DoNothing => self.push(" DO NOTHING"),
                ConflictAction::DoUpdate(assignments) => {
                    self.push(" DO UPDATE SET ");
                    self.assignments(assignments)?;
                }
            }
            Ok(())
        } else if caps.on_duplicate_key {
            match &conflict.action {
                // MySQL has no DO NOTHING; assigning a column to itself is the
                // idiomatic equivalent, so the caller must supply assignments.
                ConflictAction::DoNothing => {
                    self.unsupported("ON CONFLICT DO NOTHING; use an explicit assignment")
                }
                ConflictAction::DoUpdate(assignments) => {
                    self.push(" ON DUPLICATE KEY UPDATE ");
                    self.assignments(assignments)
                }
            }
        } else {
            self.unsupported("upsert")
        }
    }

    /// Render an UPDATE statement.
    fn update(&mut self, statement: &UpdateAst) -> RenderResult {
        self.push("UPDATE ");
        self.table_name(&statement.table);
        self.push(" SET ");
        self.assignments(&statement.assignments)?;

        if !statement.from.is_empty() {
            self.push(" FROM ");
            self.comma_separated(&statement.from, Self::source_public)?;
        }
        if let Some(filter) = &statement.filter {
            self.push(" WHERE ");
            self.expr(filter)?;
        }
        self.returning(&statement.returning)?;
        Ok(())
    }

    /// Render a DELETE statement.
    fn delete(&mut self, statement: &DeleteAst) -> RenderResult {
        self.push("DELETE FROM ");
        self.table_name(&statement.table);

        if !statement.using.is_empty() {
            self.push(" USING ");
            self.comma_separated(&statement.using, Self::source_public)?;
        }
        if let Some(filter) = &statement.filter {
            self.push(" WHERE ");
            self.expr(filter)?;
        }
        self.returning(&statement.returning)?;
        Ok(())
    }

    /// Render `SET`-style assignments.
    fn assignments(&mut self, assignments: &[Assignment]) -> RenderResult {
        self.comma_separated(assignments, |r, assignment| {
            r.ident(assignment.column);
            r.push(" = ");
            r.expr(&assignment.value)
        })
    }

    /// Render a `RETURNING` clause, refusing on dialects without one.
    fn returning(&mut self, items: &[Node]) -> RenderResult {
        if items.is_empty() {
            return Ok(());
        }
        if !self.dialect().caps().returning {
            return self.unsupported("RETURNING");
        }
        self.push(" RETURNING ");
        self.comma_separated(items, |r, node| r.expr(node))
    }

    /// A table name in a DML target position, where an alias is written without
    /// the `AS` keyword for the widest compatibility.
    fn table_name(&mut self, table: &crate::ast::node::TableRef) {
        if let Some(schema) = table.schema {
            self.ident(schema);
            self.push(".");
        }
        self.ident(table.name);
        if let Some(alias) = table.alias {
            self.push(" ");
            self.ident(alias);
        }
    }

    /// A FROM/USING source, reachable from this module.
    fn source_public(&mut self, source: &Source) -> RenderResult {
        match source {
            Source::Table(table) => {
                self.table_name(table);
                Ok(())
            }
            Source::Derived { query, alias } => {
                self.push("(");
                self.select(query)?;
                self.push(") AS ");
                self.ident(alias);
                Ok(())
            }
        }
    }
}
