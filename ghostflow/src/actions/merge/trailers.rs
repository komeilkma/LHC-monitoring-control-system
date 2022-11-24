

use log::error;

use crate::host::{Award, Comment, HostedProject, HostingServiceError, MergeRequest, User};
use crate::utils::{Trailer, TrailerRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseTrailers;

/// Markers in comments which are parsed as trailers.
const TRAILER_MARKERS: &[(&str, &[&str])] = &[
    ("Acked-by", &["+1", ":+1:", ":thumbsup:"]),
    ("Reviewed-by", &["+2"]),
    ("Tested-by", &["+3"]),
    ("Rejected-by", &["-1", ":-1:", ":thumbsdown:"]),
];

impl ParseTrailers {
    /// Find trailers from the merge request awards and comment stream.
    pub fn find(
        project: &HostedProject,
        mr: &MergeRequest,
    ) -> Result<Vec<(Trailer, Option<User>)>, HostingServiceError> {
        // Gather trailer information from the merge request.
        let comments = project.service.get_mr_comments(mr)?;
        let mr_awards = match project.service.get_mr_awards(mr) {
            Ok(awards) => awards,
            Err(err) => {
                error!(
                    target: "ghostflow/merge",
                    "failed to get awards for mr {}: {:?}",
                    mr.url,
                    err,
                );

                Vec::new()
            },
        };

        Ok(comments
            .iter()
            // Look at comments from newest to oldest.
            .rev()
            // Stop when we have a branch update comment.
            .take_while(|comment| !comment.is_branch_update)
            // Grab the trailers from user comments.
            .filter_map(|comment| {
                if comment.is_system {
                    // Ignore system comments.
                    None
                } else {
                    // Parse trailers from each comment.
                    Some(Self::parse_comment_for_trailers(project, comment))
                }
            })
            // Now that we have all the trailers, gather them up.
            .collect::<Vec<_>>()
            .into_iter()
            // Put them back into chronological order.
            .rev()
            // Put all of the trailers together into a single vector.
            .flatten()
            // Get trailers via awards on the MR itself.
            .chain({
                mr_awards
                    .into_iter()
                    .filter_map(Self::parse_award_as_trailers)
                    .collect::<Vec<_>>()
            })
            .collect())
    }

    /// Create a trailer from a user.
    fn make_user_trailer(token: &str, user: &User) -> Trailer {
        Trailer::new(token, format!("{}", user.identity()))
    }

    /// Parse an award as a trailer.
    fn parse_award_as_trailers(award: Award) -> Option<(Trailer, Option<User>)> {
        let name = &award.name;

        // Handle skin tone color variants as their base version.
        let base_name = if name[..name.len() - 1].ends_with("_tone") {
            &name[..name.len() - 6]
        } else {
            name
        };

        match base_name {
            "100" | "clap" | "tada" | "thumbsup" => {
                Some((
                    Self::make_user_trailer("Acked-by", &award.author),
                    Some(award.author),
                ))
            },
            "no_good" | "thumbsdown" => {
                Some((
                    Self::make_user_trailer("Rejected-by", &award.author),
                    Some(award.author),
                ))
            },
            _ => None,
        }
    }

    /// Parse a comment for trailers.
    fn parse_comment_for_trailers(
        project: &HostedProject,
        comment: &Comment,
    ) -> Vec<(Trailer, Option<User>)> {
        let explicit_trailers = TrailerRef::extract(&comment.content)
            .into_iter()
            // Transform values based on a some shortcuts like user references and a `me` shortcut.
            .filter_map(|trailer| {
                if !trailer.token.ends_with("-by") {
                    // Only `-by` trailers go through the username search.
                    Some((trailer.into(), None))
                } else if trailer.value.starts_with('@') {
                    // Handle user references.
                    project
                        .service
                        .user(&project.name, &trailer.value[1..])
                        // Just drop unknown user references.
                        .ok()
                        .map(|user| {
                            (
                                Self::make_user_trailer(trailer.token, &user),
                                Some(user.clone()),
                            )
                        })
                } else if trailer.value == "me" {
                    // Handle the special value `me` to mean the comment author.
                    Some((
                        Self::make_user_trailer(trailer.token, &comment.author),
                        Some(comment.author.clone()),
                    ))
                } else {
                    // Use the trailer as-is.
                    Some((trailer.into(), None))
                }
            });
        // Gather the implicit trailers from things like `+2` lines and the like.
        let implicit_trailers = comment.content.lines().filter_map(|l| {
            let line = l.trim();

            TRAILER_MARKERS
                .iter()
                .filter_map(|&(token, needles)| {
                    needles
                        .iter()
                        .find(|&needle| line.starts_with(needle))
                        .map(|_| {
                            (
                                Self::make_user_trailer(token, &comment.author),
                                Some(comment.author.clone()),
                            )
                        })
                })
                .next()
        });

        explicit_trailers.chain(implicit_trailers).collect()
    }
}
