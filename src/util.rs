// Copyright 2016 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.1.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

use futures::Future;

// Type alias for Box<Future>. Unlike `futures::BoxFuture` this doesn't require
// the future to implement `Send`.
pub type BoxFuture<T, E> = Box<Future<Item = T, Error = E>>;

// Extension methods for Future.
pub trait FutureExt: Future {
    fn into_box(self) -> BoxFuture<Self::Item, Self::Error>;
}

impl<F> FutureExt for F
where
    F: Future + 'static,
{
    fn into_box(self) -> BoxFuture<Self::Item, Self::Error> {
        Box::new(self)
    }
}
