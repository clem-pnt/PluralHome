//! Votes du système : une question, des choix, et chaque membre qui le veut
//! donne son avis. Utile pour les décisions communes (achats, rendez-vous,
//! règles de la maison).

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreError, Result, members};

pub const MAX_QUESTION_CHARS: usize = 300;
pub const MAX_OPTION_CHARS: usize = 100;
pub const MAX_OPTIONS: usize = 20;
const LIST_MAX: u32 = 500;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PollOption {
    pub id: Uuid,
    pub label: String,
    /// Membres qui ont choisi cette option.
    pub voters: Vec<Uuid>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Poll {
    pub id: Uuid,
    pub question: String,
    pub author_id: Option<Uuid>,
    /// Plusieurs choix possibles par membre.
    pub multi: bool,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
    /// Vote clos : plus personne ne peut changer son choix. Un vote dont la
    /// date de fin est passée est clos à cette date.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub closed_at: Option<i64>,
    /// Date de fin prévue (clôture automatique).
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub ends_at: Option<i64>,
    /// Le résultat a été vu : plus de notification « vote terminé ».
    pub result_seen: bool,
    pub options: Vec<PollOption>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPoll {
    pub question: String,
    pub author_id: Option<Uuid>,
    pub multi: bool,
    pub options: Vec<String>,
    /// Date de fin facultative, dans le futur.
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub ends_at: Option<i64>,
}

fn uuid(bytes: [u8; 16]) -> Uuid {
    Uuid::from_bytes(bytes)
}

fn options_of(conn: &Connection, poll: Uuid) -> Result<Vec<PollOption>> {
    let mut st = conn.prepare_cached(
        "SELECT o.id, o.label, (SELECT group_concat(hex(v.member_id)) FROM poll_vote v WHERE v.option_id = o.id)
         FROM poll_option o WHERE o.poll_id = ?1 ORDER BY o.position, o.id",
    )?;
    let rows = st.query_map([poll.as_bytes()], |r| {
        let voters: Option<String> = r.get(2)?;
        let mut voters: Vec<Uuid> = voters
            .unwrap_or_default()
            .split(',')
            .filter_map(|h| Uuid::try_parse(h).ok())
            .collect();
        voters.sort_unstable();
        Ok(PollOption {
            id: uuid(r.get(0)?),
            label: r.get(1)?,
            voters,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

const COLUMNS: &str =
    "id, question, author_id, multi, created_at, closed_at, ends_at, result_seen_at IS NOT NULL";

fn from_row(conn: &Connection, r: &rusqlite::Row<'_>) -> Result<Poll> {
    let id = uuid(r.get(0)?);
    let author: Option<[u8; 16]> = r.get(2)?;
    let closed_at: Option<i64> = r.get(5)?;
    let ends_at: Option<i64> = r.get(6)?;
    let now = crate::time::now_ms()?;
    Ok(Poll {
        id,
        question: r.get(1)?,
        author_id: author.map(uuid),
        multi: r.get(3)?,
        created_at: r.get(4)?,
        closed_at: closed_at.or(ends_at.filter(|&e| e <= now)),
        ends_at,
        result_seen: r.get(7)?,
        options: options_of(conn, id)?,
    })
}

fn get(conn: &Connection, id: Uuid) -> Result<Poll> {
    let mut st = conn.prepare_cached(&format!("SELECT {COLUMNS} FROM poll WHERE id = ?1"))?;
    let mut rows = st.query([id.as_bytes()])?;
    match rows.next()? {
        Some(r) => from_row(conn, r),
        None => Err(CoreError::NotFound),
    }
}

/// Votes en cours d'abord, puis du plus récent au plus ancien.
pub fn list(conn: &Connection) -> Result<Vec<Poll>> {
    let mut st = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM poll
         ORDER BY closed_at IS NOT NULL OR coalesce(ends_at <= ?1, 0), id DESC LIMIT {LIST_MAX}"
    ))?;
    let mut rows = st.query([crate::time::now_ms()?])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        out.push(from_row(conn, r)?);
    }
    Ok(out)
}

pub fn create(conn: &mut Connection, input: &NewPoll) -> Result<Poll> {
    let question = input.question.trim();
    if question.is_empty() || question.chars().count() > MAX_QUESTION_CHARS {
        return Err(CoreError::Invalid("question"));
    }
    let mut labels: Vec<&str> = Vec::new();
    for o in &input.options {
        let o = o.trim();
        if o.chars().count() > MAX_OPTION_CHARS {
            return Err(CoreError::Invalid("options"));
        }
        if !o.is_empty() && !labels.iter().any(|l| l.eq_ignore_ascii_case(o)) {
            labels.push(o);
        }
    }
    if labels.len() < 2 || labels.len() > MAX_OPTIONS {
        return Err(CoreError::Invalid("options"));
    }
    let now = crate::time::now_ms()?;
    if input.ends_at.is_some_and(|e| e <= now) {
        return Err(CoreError::Invalid("ends_at"));
    }
    let tx = conn.transaction()?;
    if let Some(a) = input.author_id {
        members::ensure_active(&tx, &[a])?;
    }
    let id = Uuid::now_v7();
    tx.execute(
        "INSERT INTO poll (id, question, author_id, multi, created_at, ends_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id.as_bytes(),
            question,
            input.author_id.map(|u| *u.as_bytes()),
            input.multi,
            now,
            input.ends_at
        ],
    )?;
    {
        let mut st = tx.prepare_cached(
            "INSERT INTO poll_option (id, poll_id, label, position) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for (i, label) in (0_i64..).zip(&labels) {
            st.execute(params![Uuid::now_v7().as_bytes(), id.as_bytes(), label, i])?;
        }
    }
    let p = get(&tx, id)?;
    tx.commit()?;
    Ok(p)
}

/// Enregistre le choix de ces membres (le même pour chacun) : il remplace leur
/// choix précédent. Une liste d'options vide retire leur vote.
pub fn vote(
    conn: &mut Connection,
    poll: Uuid,
    voters: &[Uuid],
    option_ids: &[Uuid],
) -> Result<Poll> {
    let tx = conn.transaction()?;
    let now = crate::time::now_ms()?;
    let (multi, closed, ends): (bool, Option<i64>, Option<i64>) = tx
        .query_row(
            "SELECT multi, closed_at, ends_at FROM poll WHERE id = ?1",
            [poll.as_bytes()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    if closed.is_some() || ends.is_some_and(|e| e <= now) {
        return Err(CoreError::Invalid("closed"));
    }
    if voters.is_empty() {
        return Err(CoreError::Invalid("voters"));
    }
    let mut chosen: Vec<Uuid> = option_ids.to_vec();
    chosen.sort_unstable();
    chosen.dedup();
    if !multi && chosen.len() > 1 {
        return Err(CoreError::Invalid("options"));
    }
    {
        let mut own =
            tx.prepare_cached("SELECT 1 FROM poll_option WHERE id = ?1 AND poll_id = ?2")?;
        for o in &chosen {
            if !own.exists(params![o.as_bytes(), poll.as_bytes()])? {
                return Err(CoreError::NotFound);
            }
        }
    }
    members::ensure_active(&tx, voters)?;
    {
        let mut clear = tx.prepare_cached(
            "DELETE FROM poll_vote WHERE member_id = ?1
             AND option_id IN (SELECT id FROM poll_option WHERE poll_id = ?2)",
        )?;
        let mut add = tx.prepare_cached(
            "INSERT INTO poll_vote (option_id, member_id, voted_at) VALUES (?1, ?2, ?3)",
        )?;
        for v in voters {
            clear.execute(params![v.as_bytes(), poll.as_bytes()])?;
            for o in &chosen {
                add.execute(params![o.as_bytes(), v.as_bytes(), now])?;
            }
        }
    }
    let p = get(&tx, poll)?;
    tx.commit()?;
    Ok(p)
}

/// Clôt ou rouvre un vote. Rouvrir retire aussi la date de fin.
pub fn set_closed(conn: &Connection, id: Uuid, closed: bool) -> Result<Poll> {
    // Qui clôt voit le résultat : pas de notification pour ce vote.
    let sql = if closed {
        "UPDATE poll SET closed_at = ?2, result_seen_at = ?2 WHERE id = ?1"
    } else {
        "UPDATE poll SET closed_at = ?2, ends_at = NULL, result_seen_at = NULL WHERE id = ?1"
    };
    let n = conn.execute(
        sql,
        params![
            id.as_bytes(),
            if closed {
                Some(crate::time::now_ms()?)
            } else {
                None
            }
        ],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    get(conn, id)
}

/// Le résultat d'un vote terminé a été vu : sa notification disparaît.
pub fn mark_result_seen(conn: &Connection, id: Uuid) -> Result<Poll> {
    let n = conn.execute(
        "UPDATE poll SET result_seen_at = coalesce(result_seen_at, ?2) WHERE id = ?1",
        params![id.as_bytes(), crate::time::now_ms()?],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: Uuid) -> Result<()> {
    if conn.execute("DELETE FROM poll WHERE id = ?1", [id.as_bytes()])? == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyfile::KdfParams;
    use crate::members::NewMember;
    use crate::vault::Vault;

    fn setup() -> (tempfile::TempDir, Vault, Uuid, Uuid) {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::new(tmp.path()).with_kdf(KdfParams::TEST);
        v.create().unwrap();
        let mk = |v: &Vault, n: &str| {
            members::create(
                v.conn().unwrap(),
                NewMember {
                    name: n.into(),
                    pronouns: None,
                    color: "#4f8583".into(),
                },
            )
            .unwrap()
            .id
        };
        let a = mk(&v, "Mila");
        let b = mk(&v, "Sacha");
        (tmp, v, a, b)
    }

    fn poll(multi: bool, options: &[&str]) -> NewPoll {
        NewPoll {
            question: "  Pizza ou pâtes ce soir ?  ".into(),
            author_id: None,
            multi,
            options: options.iter().map(|s| (*s).to_owned()).collect(),
            ends_at: None,
        }
    }

    #[test]
    fn create_validates_and_cleans() {
        let (_t, mut v, _, _) = setup();
        let c = v.conn_mut().unwrap();
        assert!(matches!(
            create(c, &poll(false, &["Pizza", " pizza ", ""])),
            Err(CoreError::Invalid("options"))
        ));
        let mut empty = poll(false, &["a", "b"]);
        empty.question = " ".into();
        assert!(matches!(
            create(c, &empty),
            Err(CoreError::Invalid("question"))
        ));
        let p = create(c, &poll(false, &["Pizza", "Pâtes", "pizza", " "])).unwrap();
        assert_eq!(p.question, "Pizza ou pâtes ce soir ?");
        let labels: Vec<_> = p.options.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(labels, ["Pizza", "Pâtes"]);
    }

    #[test]
    fn vote_replace_close_and_delete() {
        let (_t, mut v, mila, sacha) = setup();
        let c = v.conn_mut().unwrap();
        let p = create(c, &poll(false, &["Pizza", "Pâtes"])).unwrap();
        let (pizza, pates) = (p.options[0].id, p.options[1].id);

        assert!(matches!(
            vote(c, p.id, &[mila], &[pizza, pates]),
            Err(CoreError::Invalid("options"))
        ));
        let after = vote(c, p.id, &[mila, sacha], &[pizza]).unwrap();
        assert_eq!(after.options[0].voters.len(), 2);
        // Changer d'avis remplace le vote précédent.
        let after = vote(c, p.id, &[mila], &[pates]).unwrap();
        assert_eq!(after.options[0].voters, vec![sacha]);
        assert_eq!(after.options[1].voters, vec![mila]);
        let after = vote(c, p.id, &[sacha], &[]).unwrap();
        assert!(after.options[0].voters.is_empty());

        // Une option d'un autre vote est refusée.
        let other = create(c, &poll(true, &["A", "B", "C"])).unwrap();
        assert!(matches!(
            vote(c, p.id, &[mila], &[other.options[0].id]),
            Err(CoreError::NotFound)
        ));
        let multi = vote(
            c,
            other.id,
            &[mila],
            &[other.options[0].id, other.options[2].id],
        )
        .unwrap();
        assert_eq!(
            multi
                .options
                .iter()
                .filter(|o| !o.voters.is_empty())
                .count(),
            2
        );

        let closed = set_closed(c, p.id, true).unwrap();
        assert!(closed.closed_at.is_some() && closed.result_seen);
        assert!(matches!(
            vote(c, p.id, &[sacha], &[pizza]),
            Err(CoreError::Invalid("closed"))
        ));
        // Les votes en cours passent avant les votes clos.
        let all = list(c).unwrap();
        assert_eq!(all[0].id, other.id);
        assert_eq!(all[1].id, p.id);
        set_closed(c, p.id, false).unwrap();

        delete(c, p.id).unwrap();
        assert!(matches!(delete(c, p.id), Err(CoreError::NotFound)));
        assert_eq!(list(c).unwrap().len(), 1);
    }

    #[test]
    fn end_date_closes_the_poll() {
        let (_t, mut v, mila, _) = setup();
        let c = v.conn_mut().unwrap();
        let now = crate::time::now_ms().unwrap();
        let mut past = poll(false, &["A", "B"]);
        past.ends_at = Some(now - 1000);
        assert!(matches!(
            create(c, &past),
            Err(CoreError::Invalid("ends_at"))
        ));

        let mut later = poll(false, &["A", "B"]);
        later.ends_at = Some(now + 3_600_000);
        let p = create(c, &later).unwrap();
        assert_eq!(p.ends_at, later.ends_at);
        assert!(p.closed_at.is_none());
        vote(c, p.id, &[mila], &[p.options[0].id]).unwrap();

        // L'échéance passe : le vote est clos à cette date.
        let ended = now - 10;
        c.execute(
            "UPDATE poll SET ends_at = ?2 WHERE id = ?1",
            params![p.id.as_bytes(), ended],
        )
        .unwrap();
        let open = create(c, &poll(false, &["C", "D"])).unwrap();
        let all = list(c).unwrap();
        assert_eq!(all[0].id, open.id);
        assert_eq!(all[1].closed_at, Some(ended));
        assert!(!all[1].result_seen);
        assert!(mark_result_seen(c, p.id).unwrap().result_seen);
        assert!(matches!(
            vote(c, p.id, &[mila], &[p.options[1].id]),
            Err(CoreError::Invalid("closed"))
        ));

        // Rouvrir retire l'échéance.
        let reopened = set_closed(c, p.id, false).unwrap();
        assert!(reopened.closed_at.is_none() && reopened.ends_at.is_none());
        assert!(!reopened.result_seen);
        vote(c, p.id, &[mila], &[p.options[1].id]).unwrap();
    }
}
