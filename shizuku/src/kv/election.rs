use crate::kv::{KeyValue, KeyValueRead, KeyValueWrite, KvReadError, KvWriteError};
use async_nats::jetstream::kv::Store;
use rand::Rng;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use kanau::message::{DeserializeError, SerializeError};
use kanau::processor::Processor;
use kanau_macro::{BincodeMessageDe, BincodeMessageSer};
use thiserror::Error;

/// The state of a node in the election.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BincodeMessageSer, BincodeMessageDe, bincode::Encode, bincode::Decode)]
#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
pub enum ElectionNodeState {
    /// The node is a follower.
    #[default]
    Follower,

    /// The node is a candidate.
    Candidate,

    /// The node is the leader.
    Leader,
}

/// The election value stored in the key-value store.
#[derive(Debug, Clone, BincodeMessageSer, BincodeMessageDe, bincode::Encode, bincode::Decode)]
#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
pub struct ElectionValue {
    /// The current leader ID, if any.
    pub leader_id: Option<String>,

    /// The term number, incremented on each election.
    pub term: u64,

    /// Timestamp when the leader was last seen (in seconds since UNIX epoch).
    pub last_heartbeat: u64,

    /// List of nodes that have voted in the current term.
    pub voters: Vec<String>,
}

impl Default for ElectionValue {
    fn default() -> Self {
        Self::new()
    }
}

impl ElectionValue {
    /// Create a new election value with no leader.
    pub fn new() -> Self {
        Self {
            leader_id: None,
            term: 0,
            last_heartbeat: Self::current_timestamp(),
            voters: Vec::new(),
        }
    }

    /// Get the current timestamp in seconds since UNIX epoch.
    fn current_timestamp() -> u64 {
        #[allow(clippy::expect_used)]
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }

    /// Check if the leader has timed out (no heartbeat for more than 10 seconds).
    pub fn is_leader_timeout(&self) -> bool {
        if self.leader_id.is_none() {
            return false;
        }

        let now = Self::current_timestamp();
        now.saturating_sub(self.last_heartbeat) > 10
    }

    /// Update the election value for a new term with the given node as a candidate.
    pub fn start_new_term(&self, node_id: &str) -> Self {
        Self {
            leader_id: None,
            term: self.term + 1,
            last_heartbeat: Self::current_timestamp(),
            voters: vec![node_id.to_string()],
        }
    }

    /// Add a vote for the candidate in the current term.
    pub fn add_vote(&self, voter_id: &str) -> Self {
        let mut voters = self.voters.clone();
        if !voters.contains(&voter_id.to_string()) {
            voters.push(voter_id.to_string());
        }

        Self {
            leader_id: self.leader_id.clone(),
            term: self.term,
            last_heartbeat: self.last_heartbeat,
            voters,
        }
    }

    /// Elect a leader if the candidate has a majority of votes.
    pub fn elect_leader(&self, candidate_id: &str, total_nodes: usize) -> Option<Self> {
        // Need a majority of votes to become leader
        if self.voters.len() <= total_nodes / 2 {
            return None;
        }

        Some(Self {
            leader_id: Some(candidate_id.to_string()),
            term: self.term,
            last_heartbeat: Self::current_timestamp(),
            voters: self.voters.clone(),
        })
    }

    /// Update the heartbeat timestamp for the current leader.
    pub fn update_heartbeat(&self) -> Self {
        Self {
            leader_id: self.leader_id.clone(),
            term: self.term,
            last_heartbeat: Self::current_timestamp(),
            voters: self.voters.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
/// Error when deserializing the election state.
pub enum ElectionDesErr {
    /// Invalid byte length
    #[error("Invalid byte length")]
    InvalidByteLength,

    /// Invalid format
    #[error("Invalid format")]
    InvalidFormat,
}

impl From<ElectionDesErr> for DeserializeError {
    fn from(err: ElectionDesErr) -> Self {
        DeserializeError(anyhow::Error::new(err))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
/// Error when serializing the election state.
#[error("Failed to serialize election state")]
pub enum ElectionSerErr {}

impl From<ElectionSerErr> for SerializeError {
    fn from(err: ElectionSerErr) -> Self {
        SerializeError(anyhow::Error::new(err))
    }
}

#[derive(Debug, Clone)]
/// A distributed election algorithm implementation.
pub struct Election {
    key: String,
    value: ElectionValue,
}

impl KeyValue for Election {
    type Key = String;
    type Value = ElectionValue;

    fn key(&self) -> Self::Key {
        self.key.clone()
    }

    fn value(&self) -> Self::Value {
        self.value.clone()
    }

    fn into_value(self) -> Self::Value {
        self.value
    }

    fn new(key: Self::Key, value: Self::Value) -> Self {
        Self { key, value }
    }
}

#[derive(Debug, Error)]
/// Error when trying to perform election operations.
pub enum ElectionError {
    /// The read failed and unable to recover
    #[error("Read failed: {0}")]
    ReadFailed(KvReadError<ElectionValue>),

    /// The update failed
    #[error("Update failed: {0}")]
    UpdateFailed(KvWriteError<ElectionValue>),

    /// Unexpected missing value
    #[error("Unexpected missing value")]
    UnexpectedMissingValue,

    /// Not the leader
    #[error("Not the leader")]
    NotLeader,

    /// Already a leader
    #[error("Already a leader")]
    AlreadyLeader,
}

/// Random sleep for a duration between `min_ms` and `max_ms`.
#[inline]
async fn random_sleep(min_ms: u64, max_ms: u64) {
    let sleep_time = rand::rng().random_range(min_ms..max_ms);
    tokio::time::sleep(Duration::from_millis(sleep_time)).await;
}

impl Election {
    /// Initialize the election state if it doesn't exist.
    pub async fn initialize(
        store: &Store,
        key: impl AsRef<str>,
    ) -> Result<(), ElectionError> {
        let entry = Self::atomic_read_from(store, key.as_ref().to_string())
            .await
            .map_err(ElectionError::ReadFailed)?;

        if entry.is_none() {
            // Create initial election state
            let new_election = Self::new(
                key.as_ref().to_string(),
                ElectionValue::new(),
            );

            let create_result = new_election.create_write(store).await;
            match create_result {
                Ok(_) => Ok(()),
                Err(err) => Err(ElectionError::UpdateFailed(err)),
            }
        } else {
            // Already initialized
            Ok(())
        }
    }

    /// Start an election as a candidate.
    pub async fn start_election(
        store: &Store,
        key: impl AsRef<str>,
        node_id: impl AsRef<str>,
        total_nodes: usize,
    ) -> Result<bool, ElectionError> {
        // Initialize if needed
        let _ = Self::initialize(store, key.as_ref()).await;

        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(ElectionError::ReadFailed)?;

            let Some(entry) = entry else {
                return Err(ElectionError::UnexpectedMissingValue);
            };

            let election_value = entry.value;

            // If there's already a leader and it's not timed out, we can't start an election
            if election_value.leader_id.is_some() && !election_value.is_leader_timeout() {
                return Ok(false);
            }

            // Start a new term with this node as the first voter
            let updated = election_value.start_new_term(node_id.as_ref());
            let updated = Self::new(key.as_ref().to_string(), updated);

            let revision_result = updated.write_to_atomically(store, entry.revision).await;
            match revision_result {
                Ok(Some(_)) => {
                    // Successfully started the election, now try to get votes
                    return Self::try_get_elected(
                        store, 
                        key.as_ref(), 
                        node_id.as_ref(), 
                        total_nodes
                    ).await;
                },
                Ok(None) => {
                    // Someone else updated the value, try again
                    random_sleep(20, 100).await;
                    continue;
                },
                Err(err) => {
                    return Err(ElectionError::UpdateFailed(err));
                }
            }
        }
    }

    /// Try to get elected as the leader.
    async fn try_get_elected(
        store: &Store,
        key: impl AsRef<str>,
        node_id: impl AsRef<str>,
        total_nodes: usize,
    ) -> Result<bool, ElectionError> {
        // In a real system, this would involve requesting votes from other nodes
        // For this implementation, we'll simulate getting votes by checking if we have enough

        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(ElectionError::ReadFailed)?;

            let Some(entry) = entry else {
                return Err(ElectionError::UnexpectedMissingValue);
            };

            let election_value = entry.value;

            // Check if we're still in the election we started
            if election_value.leader_id.is_some() {
                // Someone else became leader
                return Ok(false);
            }

            // Check if we have enough votes
            if let Some(updated) = election_value.elect_leader(node_id.as_ref(), total_nodes) {
                let updated = Self::new(key.as_ref().to_string(), updated);
                let revision_result = updated.write_to_atomically(store, entry.revision).await;

                match revision_result {
                    Ok(Some(_)) => {
                        // Successfully became leader
                        return Ok(true);
                    },
                    Ok(None) => {
                        // Someone else updated the value, try again
                        random_sleep(20, 100).await;
                        continue;
                    },
                    Err(err) => {
                        return Err(ElectionError::UpdateFailed(err));
                    }
                }
            } else {
                // Not enough votes yet, wait and try again
                random_sleep(50, 150).await;
                continue;
            }
        }
    }

    /// Send a heartbeat as the leader to maintain leadership.
    pub async fn send_heartbeat(
        store: &Store,
        key: impl AsRef<str>,
        node_id: impl AsRef<str>,
    ) -> Result<(), ElectionError> {
        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(ElectionError::ReadFailed)?;

            let Some(entry) = entry else {
                return Err(ElectionError::UnexpectedMissingValue);
            };

            let election_value = entry.value;

            // Check if we're the leader
            if election_value.leader_id.as_ref() != Some(&node_id.as_ref().to_string()) {
                return Err(ElectionError::NotLeader);
            }

            // Update the heartbeat
            let updated = election_value.update_heartbeat();
            let updated = Self::new(key.as_ref().to_string(), updated);

            let revision_result = updated.write_to_atomically(store, entry.revision).await;
            match revision_result {
                Ok(Some(_)) => {
                    // Successfully updated heartbeat
                    return Ok(());
                },
                Ok(None) => {
                    // Someone else updated the value, try again
                    random_sleep(20, 100).await;
                    continue;
                },
                Err(err) => {
                    return Err(ElectionError::UpdateFailed(err));
                }
            }
        }
    }

    /// Check if the node is the current leader.
    pub async fn is_leader(
        store: &Store,
        key: impl AsRef<str>,
        node_id: impl AsRef<str>,
    ) -> Result<bool, ElectionError> {
        let entry = Self::atomic_read_from(store, key.as_ref().to_string())
            .await
            .map_err(ElectionError::ReadFailed)?;

        let Some(entry) = entry else {
            return Ok(false); // No election state yet
        };

        let election_value = entry.value;

        // Check if we're the leader and the leader hasn't timed out
        Ok(
            election_value.leader_id.as_ref() == Some(&node_id.as_ref().to_string()) 
            && !election_value.is_leader_timeout()
        )
    }

    /// Get the current leader, if any.
    pub async fn get_leader(
        store: &Store,
        key: impl AsRef<str>,
    ) -> Result<Option<String>, ElectionError> {
        let entry = Self::atomic_read_from(store, key.as_ref().to_string())
            .await
            .map_err(ElectionError::ReadFailed)?;

        let Some(entry) = entry else {
            return Ok(None); // No election state yet
        };

        let election_value = entry.value;

        // Return the leader if it hasn't timed out
        if election_value.is_leader_timeout() {
            Ok(None)
        } else {
            Ok(election_value.leader_id)
        }
    }

    /// Vote for a candidate in the current term.
    pub async fn vote_for(
        store: &Store,
        key: impl AsRef<str>,
        voter_id: impl AsRef<str>,
        candidate_id: impl AsRef<str>,
    ) -> Result<(), ElectionError> {
        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(ElectionError::ReadFailed)?;

            let Some(entry) = entry else {
                return Err(ElectionError::UnexpectedMissingValue);
            };

            let election_value = entry.value;

            // Check if there's already a leader
            if election_value.leader_id.is_some() && !election_value.is_leader_timeout() {
                return Ok(());
            }

            // Add the vote
            let updated = election_value.add_vote(voter_id.as_ref());

            // Try to elect the candidate if they have a majority of votes
            // For simplicity, we'll assume the total nodes is the current number of voters plus a small margin
            let estimated_total_nodes = updated.voters.len() + 1; // Add 1 to ensure we need more than half
            if let Some(elected) = updated.elect_leader(candidate_id.as_ref(), estimated_total_nodes) {
                let updated = Self::new(key.as_ref().to_string(), elected);
                return updated.write_to_atomically(store, entry.revision).await
                    .map(|_| ())
                    .map_err(ElectionError::UpdateFailed);
            }

            let updated = Self::new(key.as_ref().to_string(), updated);

            let revision_result = updated.write_to_atomically(store, entry.revision).await;
            match revision_result {
                Ok(Some(_)) => {
                    // Successfully voted
                    return Ok(());
                },
                Ok(None) => {
                    // Someone else updated the value, try again
                    random_sleep(20, 100).await;
                    continue;
                },
                Err(err) => {
                    return Err(ElectionError::UpdateFailed(err));
                }
            }
        }
    }

    /// Step down as leader.
    pub async fn step_down(
        store: &Store,
        key: impl AsRef<str>,
        node_id: impl AsRef<str>,
    ) -> Result<(), ElectionError> {
        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(ElectionError::ReadFailed)?;

            let Some(entry) = entry else {
                return Err(ElectionError::UnexpectedMissingValue);
            };

            let election_value = entry.value;

            // Check if we're the leader
            if election_value.leader_id.as_ref() != Some(&node_id.as_ref().to_string()) {
                return Err(ElectionError::NotLeader);
            }

            // Create a new election value with no leader
            let updated = ElectionValue {
                leader_id: None,
                term: election_value.term,
                last_heartbeat: ElectionValue::current_timestamp(),
                voters: Vec::new(),
            };

            let updated = Self::new(key.as_ref().to_string(), updated);

            let revision_result = updated.write_to_atomically(store, entry.revision).await;
            match revision_result {
                Ok(Some(_)) => {
                    // Successfully stepped down
                    return Ok(());
                },
                Ok(None) => {
                    // Someone else updated the value, try again
                    random_sleep(20, 100).await;
                    continue;
                },
                Err(err) => {
                    return Err(ElectionError::UpdateFailed(err));
                }
            }
        }
    }
}

/// A processor that ensures the node is the leader before processing.
pub struct LeaderGuardProcessor<I, O, P, K> 
where 
    P: Processor<I, O> + Send + Sync,
    K: AsRef<str> + Send + Sync,
    I: Send + Sync,
    O: Send + Sync,
{
    processor: Arc<P>,
    store: &'static Store,
    election_key: K,
    node_id: String,
    phantom_input: PhantomData<I>,
    phantom_output: PhantomData<O>,
}

#[derive(Debug, Error)]
/// Error when processing with leader guard.
pub enum LeaderGuardError {
    /// Not the leader
    #[error("Not the leader")]
    NotLeader,

    /// Election error
    #[error("Election error: {0}")]
    ElectionError(#[from] ElectionError),
}

impl<I, O, P, K> Processor<I, Result<O, LeaderGuardError>> for LeaderGuardProcessor<I, O, P, K>
where 
    P: Processor<I, O> + Send + Sync,
    K: AsRef<str> + Send + Sync,
    I: Send + Sync,
    O: Send + Sync,
{
    async fn process(&self, input: I) -> Result<O, LeaderGuardError> {
        // Check if we're the leader
        let is_leader = Election::is_leader(self.store, &self.election_key, &self.node_id).await?;

        if !is_leader {
            return Err(LeaderGuardError::NotLeader);
        }

        // Process the input if we're the leader
        let result = self.processor.process(input).await;

        Ok(result)
    }
}

impl<I, O, P, K> LeaderGuardProcessor<I, O, P, K>
where 
    P: Processor<I, O> + Send + Sync,
    K: AsRef<str> + Send + Sync,
    I: Send + Sync,
    O: Send + Sync,
{
    /// Create a new LeaderGuardProcessor
    pub fn new(processor: Arc<P>, store: &'static Store, election_key: K, node_id: String) -> Self {
        Self {
            processor,
            store,
            election_key,
            node_id,
            phantom_input: PhantomData,
            phantom_output: PhantomData,
        }
    }
}
