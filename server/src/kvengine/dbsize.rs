/*
 * Created on Thu Sep 24 2020
 *
 * This file is a part of Skytable
 * Skytable (formerly known as TerrabaseDB or Skybase) is a free and open-source
 * NoSQL database written by Sayan Nandan ("the Author") with the
 * vision to provide flexibility in data modelling without compromising
 * on performance, queryability or scalability.
 *
 * Copyright (c) 2020, Sayan Nandan <ohsayan@outlook.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 *
*/

use crate::dbnet::con::prelude::*;
use crate::protocol::responses;
use crate::resp::GroupBegin;


/// Get the number of keys in the database
pub async fn dbsize<T, Strm>(
    handle: &crate::coredb::CoreDB,
    con: &mut T,
    act: crate::protocol::ActionGroup,
) -> std::io::Result<()>
where
    T: ProtocolConnectionExt<Strm>,
    Strm: AsyncReadExt + AsyncWriteExt + Unpin + Send + Sync,
{
    if act.howmany() != 0 {
        return con.write_response(&**responses::fresp::R_ACTION_ERR).await;
    }
    let len;
    {
        len = handle.acquire_read().get_ref().len();
    }
    con.write_response(GroupBegin(1)).await?;
    con.write_response(len).await?;
    Ok(())
}
