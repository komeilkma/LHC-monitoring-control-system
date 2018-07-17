<?php
include("includes/controller.php");

// This is a modal box for usergroups.php - show warning that admin is signed out.
if (!$session->isAdmin()) {
    echo "You are no longer logged in.";
    exit;
} else {
    
if (!empty($_GET['log_id'])) {
    $logid = $_GET['log_id'];
    $groupinfo = $functions->returnGroupInfo($db, $logid);
} else {
    header("Location: " . $configs->homePage());
    exit;
}
// Protect Administrators Group from Non Super Admin
if (($_GET['log_id'] == '1') && !$session->isSuperAdmin()) {
    header("Location: " . $configs->homePage());
    exit;
}
?>
            
<div class="modal-content" id="modal-content">
    <form action="includes/adminprocess.php" id="user-groups-edit" class="form-horizontal" method="post">
        <div class="modal-header">
            <button type="button" class="close" data-dismiss="modal" aria-hidden="true">&times;</button>
            <h4 class="modal-title" id="myModalLabel">Edit Group</h4>
        </div>
        <div class="modal-body" id="modal-body">                                   

            <div class="form-group">
                <label for="sitedesc" class="col-sm-3 control-label">Group Name : </label>
                <div class="col-md-8">
                    <input type="text" name="group_name" class="form-control" placeholder="Group Name" value="<?php echo $groupinfo['group_name']; ?>" <?php if ($groupinfo["group_name"] == 'Administrators') { echo 'disabled'; } ?> />
                </div>
            </div>
            <div class="form-group">
                <label for="group_level" class="col-sm-3 control-label">Group Level : </label>
                <div class="col-md-8">
                    <input type="text" name="group_level" id="group_level" class="form-control" placeholder="Group Level" value="<?php echo $groupinfo["group_level"]; ?>" <?php if ($groupinfo["group_level"] == '1') { echo 'disabled'; } ?> />
                </div>
            </div>
            
            <div class="form-group">
                <label for="add-user" class="col-sm-3 control-label" >Add Users : </label>
                <div class="col-md-8">
                    <select for="add-user" name="add-user[]" class="chosen-select" data-placeholder="Click here.." style="width: 250px;" multiple>
                        <option></option><!-- Required for data-placeholder attribute to work with Chosen plugin -->
                        <?php
                        $sql = "SELECT id, username FROM `users` WHERE users.id NOT IN (SELECT user_id FROM users_groups WHERE users_groups.group_id = '$logid' )";
                        $result = $db->prepare($sql);
                        $result->execute();
                        while ($row = $result->fetch()) {
                            echo "<option value='" . $row['id'] . "'>" . $row['username'] . "</option>";
                        }
                        ?>
                    </select>
                </div>
            </div>
                            
            <div class="row">                                     
                <div class="col-sm-12 col-md-12">
                    <div class="panel">
                        <div class="panel-body">
                            <table class="table table-striped table-bordered table-hover" id="dataTable2">
                                <thead>
                                    <tr>
                                        <th>Username</th>
                                        <th class='text-center'>Remove</th>
                                    </tr>
                                </thead>
                                <tbody>
                            <?php
                            $sql = "SELECT users.username, users.id, users_groups.group_id FROM `users` INNER JOIN `users_groups` ON users.id=users_groups.user_id WHERE users_groups.group_id = '$logid' ORDER BY users.username ASC";
                            $result = $db->prepare($sql);
                            $result->execute();
                            $stop = $adminfunctions->createStop($session->username, 'delete-groupmembership');
                            while ($row = $result->fetch()) {
                                echo "<tr><td>" . $row['username'] . "</td>";
                                echo "<td class='text-center'><div class='btn-group btn-group-xs'>";
                                if ($row['username'] != ADMIN_NAME) {
                                    echo "<a href='includes/adminprocess.php?remove=" . $row['id'] . "&group_id=" . $row['group_id'] . "&stop=" . $stop . "&form_submission=remove_groupmember' title='Remove Group Member' class='btn btn-danger confirmation_deleteuser'>";
                                    echo "<i class='fa fa-times'></i></a></div>";
                                }
                                echo "</td></tr>";
                            }
                            ?>
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>
            </div>

            <input type="hidden" name="form_submission" value="edit_group">
            <input type="hidden" name="group_id" value="<?php echo $logid; ?>"> 

        </div>
        <div class="modal-footer">
            <button type="button" class="btn btn-default" data-dismiss="modal">Close</button>
            <button type="submit" class="btn btn-primary" id="submit" >Edit Group</button>
        </div>
    </form>
</div>

        <!-- Datatables JS - https://cdn.datatables.net/ -->
        <script src="js/plugins/datatables/jquery.dataTables.min.js"></script>
        <script src="js/plugins/datatables/dataTables.bootstrap.min.js"></script>
        <script>
        $(document).ready(function(){
            $('#dataTable2').DataTable();
        }); 
        </script>
        
        <!-- Initialize Form Validation -->
        <script src="js/plugins/formValidation/userGroupsFormsValidation.js"></script>
        <script src="js/plugins/formValidation/jquery.validate.js"></script>
        <script>$(function() { FormsValidation.init(); });</script> 
        
        <!-- Chosen JS - https://harvesthq.github.io/chosen/ -->
        <script src="js/plugins/chosen/chosen.js"></script>
        <script>
            $(".chosen-select").chosen({ width: '100%' }).change(function() {
                var v = $(this).val();
            });
        </script>
    
        <script type="text/javascript">
            $('.confirmation_deleteuser').on('click', function () {
            return confirm('Are you sure you wish to delete the user from this group?');
            });
        </script> 
    
<?php } ?>